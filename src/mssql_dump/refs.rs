use super::*;

#[allow(dead_code)]
pub(super) fn build_metadata_command_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, MetadataCommandReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_metadata_command_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_metadata_command_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, MetadataCommandReference> {
    let mut index = BTreeMap::new();
    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        let use_standard_commands =
            metadata_use_standard_commands(kind, &row.text, header).unwrap_or(true);
        let based_on_declared = metadata_based_on_declared(kind, &row.text, header);
        index.insert(
            row.file_name.clone(),
            MetadataCommandReference {
                kind: kind.to_string(),
                name: header.name.clone(),
                use_standard_commands,
                based_on_declared,
            },
        );
    }
    index
}

/// The target's own `<UseStandardCommands>`, read through the same decoder and
/// at the same slot the kind's own properties parser uses -- offset 31 of the
/// normalized owner fields for `Catalog`, offset 23 for `Document`, logical
/// field 7 for `InformationRegister`, and slot 7 of the object fields for
/// `Report`. `None` for every other kind, or when the decode fails: the caller
/// then keeps the pre-existing `true` assumption rather than a guessed offset,
/// per the project's fail-closed rule on unevidenced field positions.
///
/// `Document` and `Report` were added on a corpus-wide census rather than a
/// single file. Every `code:uuid` sentinel the platform writes in a
/// `Ext/CommandInterface.xml` of ERP УХ 3.2.12.6 belongs to one of four kinds
/// -- `Catalog` (6 targets), `InformationRegister` (12), `Document` (5),
/// `Report` (2) -- and the first two were the only ones this function could
/// answer for, so the last two synthesized a `.StandardCommand.X` name over
/// the platform's sentinel in seven `Subsystems/.../Ext/CommandInterface.xml`
/// files. The converse direction is clean across the whole stand: over ERP УХ,
/// UT, Документооборот and БСП demo, of the 904 documents and reports that
/// declare `<UseStandardCommands>false</UseStandardCommands>` not one is named
/// with a `.StandardCommand.` anywhere in the platform's own trees, so reading
/// the declaration can only remove synthesized names the platform never wrote.
fn metadata_use_standard_commands(kind: &str, text: &str, header: &MetadataHeader) -> Option<bool> {
    if kind == "InformationRegister" {
        // Same slot `parse_information_register_owner_properties` already
        // reads for the register's own `InformationRegisters/<name>.xml`
        // (`UseStandardCommands`, logical field 7). Confirmed missing here
        // specifically on ERP УХ 3.2.12.6,
        // `InformationRegisters/СоответствиеВнутригрупповыхПоказателей`
        // (`UseStandardCommands=false`): every `Subsystems/.../Ext/
        // CommandInterface.xml` referencing its `StandardCommand.OpenList`
        // synthesized the name instead of keeping the platform's own
        // `0:<uuid>` sentinel, because this function returned `None` (falling
        // back to the default `true`) for every kind but `Catalog`.
        let fields = parse_information_register_owner_fields(text, header)?;
        return information_register_bool(fields.logical.get(7)?);
    }
    if kind == "Report" {
        // Same slot `parse_report_properties_from_text` reads for the report's
        // own `Reports/<name>.xml`, behind the same layout-marker guard it
        // uses: a marker this reader does not know is a refusal, not a slot 7
        // read blind.
        let fields = metadata_object_fields(text)?;
        if !matches!(fields.first().map(|value| value.trim()), Some("19" | "20")) {
            return None;
        }
        return parse_1c_bool_field(fields.get(7).copied());
    }
    let family = match kind {
        "Catalog" => owner_graph::OwnerGraphFamily::Catalog,
        // Same slot `parse_document_properties_from_text` reads for the
        // document's own `Documents/<name>.xml`, through the same owner-graph
        // decoder.
        "Document" => owner_graph::OwnerGraphFamily::Document,
        _ => return None,
    };
    let slot = match kind {
        "Catalog" => 31,
        _ => 23,
    };
    let mut diagnostic = None;
    let graph = decode_owner_graph_for_family_parser(family, text, header, &mut diagnostic)?;
    information_register_bool(graph.owner_fields.get(slot)?)
}

/// How many members the target's own `<BasedOn>` list declares, read at the
/// slot the kind's own properties parser reads it from -- offset 22 of the
/// normalized owner fields for `Document`, offset 32 for `Catalog`, the two
/// families this decoder answers for and the only two the stand's
/// `CreateBasedOn` targets belong to. `None` for every other kind and whenever
/// the slot is not the counted reference collection every one of these
/// properties shares: an unread declaration withholds nothing and the caller
/// keeps naming the command exactly as before.
fn metadata_based_on_declared(kind: &str, text: &str, header: &MetadataHeader) -> Option<usize> {
    let (family, slot) = match kind {
        "Catalog" => (owner_graph::OwnerGraphFamily::Catalog, 32),
        "Document" => (owner_graph::OwnerGraphFamily::Document, 22),
        _ => return None,
    };
    let mut diagnostic = None;
    let graph = decode_owner_graph_for_family_parser(family, text, header, &mut diagnostic)?;
    metadata_reference_collection_len(graph.owner_fields.get(slot)?)
}

/// What a metadata table declares about the existence of its own standard
/// attributes.
///
/// A dynamic list resolves a remembered field name against the fields its main
/// table has, and a standard attribute the table does not declare is not one of
/// them: a catalog has no `Code` when `<CodeLength>` is zero, no `Description`
/// when `<DescriptionLength>` is, no `Owner` when `<Owners>` names nobody, no
/// `Parent` when it is not hierarchical and no `IsFolder` unless its hierarchy
/// holds folders; an information register has no `Period` when it is
/// `Nonperiodical`.
///
/// Every property is `None` when this reader could not name it, and an unread
/// property withholds nothing: the attribute stays in the list's universe
/// exactly as before, so a family this index does not decode behaves as it did.
///
/// Refused rather than guessed: `Number` on a document, business process or
/// task whose `<NumberLength>` is zero, and `Recorder`/`LineNumber` on an
/// independent information register. Both are the same shape as the rules
/// above, and neither has a single platform observation on the stand -- over
/// the whole corpus the two would have withdrawn a name from 446 ERP УХ and 240
/// Документооборот lists without changing one written marker.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct MetadataTableStandardAttributes {
    code_length: Option<u32>,
    description_length: Option<u32>,
    /// Whether `<Owners>` declares at least one owner.
    owned: Option<bool>,
    hierarchical: Option<bool>,
    /// Whether `<HierarchyType>` is `HierarchyFoldersAndItems`.
    folder_hierarchy: Option<bool>,
    /// Whether `<InformationRegisterPeriodicity>` is anything but
    /// `Nonperiodical`.
    periodical: Option<bool>,
}

impl MetadataTableStandardAttributes {
    /// Whether the table declares the standard attribute its family spells
    /// `english`. A name this index knows no property for is declared.
    pub(super) fn declares(&self, english: &str) -> bool {
        match english {
            "Code" => self.code_length != Some(0),
            "Description" => self.description_length != Some(0),
            "Owner" => self.owned != Some(false),
            "Parent" => self.hierarchical != Some(false),
            "IsFolder" => self.hierarchical != Some(false) && self.folder_hierarchy != Some(false),
            "Period" => self.periodical != Some(false),
            _ => true,
        }
    }
}

/// A common attribute's declared content: the tables it is a field of.
///
/// `<Content>` names the tables that opt in or out one by one and `<AutoUse>`
/// settles the rest. A common attribute is a field of a dynamic list's main
/// table only when the content puts it there -- ERP УХ 3.2.12.6 marks
/// `~Список.КлассВНА` on `Catalog.ГруппыВНАМСФО`,
/// `Document.ИзменениеПараметровВНАМСФО` and
/// `Document.ВводНачальныхОстатковВНАМСФО`, none of which
/// `CommonAttribute.КлассВНА` lists, and `~Список.НСИ_НеАктивный` on
/// `Catalog.ПроизвольныйКлассификаторУХ` for the same reason.
///
/// An `<AutoUse>` this reader cannot name is a refusal: the declaration is left
/// out of the index entirely and the attribute is admitted everywhere, as
/// before.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct MetadataCommonAttributeContent {
    auto_use: bool,
    content: BTreeMap<String, bool>,
}

impl MetadataCommonAttributeContent {
    pub(super) fn covers(&self, table: &str) -> bool {
        self.content.get(table).copied().unwrap_or(self.auto_use)
    }
}

/// What one constant declares about its own place in a form's constants set.
///
/// `always_used` is the constant record's own slot 11, the one flag of the
/// owner record no exported property carries: `uh` `ВсегдаКонтролироватьБаланс`
/// `РучныхОпераций` and `ПодставлятьЗначенияПоУмолчаниюВместоПустых` are both
/// booleans whose `Constants/<name>.xml` agree in every element but the name,
/// and the two disagree here.
///
/// A form's `<UseAlways>` record does not hold the set of always-used fields --
/// it holds the fields whose flag *differs* from this declaration. Evidence:
/// ERP УХ 3.2.12.6, every one of the 84 `cfg:ConstantsSet` attributes on the
/// configuration joined against the platform's own `<UseAlways>`. 345 of the
/// 353 constants those records name are written exactly where the record names
/// them; the eight that are not are exactly the eight whose slot 11 is `1` or
/// whose declared type is `v8:ValueStorage`, and both behave as the delta
/// reading predicts on every one of the 84 forms:
///
///   * `ВсегдаКонтролироватьБалансРучныхОпераций` and `ПутьККаталогуИмпорта`
///     (slot 11 `1`) are written in the 69 forms whose record leaves them out
///     and in none of the 15 whose record names them;
///   * `СрокОплатыПокупателей` and `СрокОплатыПоставщикам` (slot 11 `1`) the
///     same way, 77 against 7;
///   * `ДополнительныеЯзыкиВыводаОтчета`, `НастройкиКолонтитуловПоУмолчанию`,
///     `ПараметрыАдминистрированияИБ` and `СтатусОбновленияКонфигурации` are
///     never written at all, and those four are `v8:ValueStorage`.
///
/// `value_storage` is that second rule. Over the whole stand -- 3 378
/// constants, 225 of them `v8:ValueStorage` -- the platform writes 768
/// `ConstantsSet` use-always fields and not one names a `ValueStorage`
/// constant.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct MetadataConstantDeclaration {
    pub(super) always_used: bool,
    pub(super) value_storage: bool,
}

/// The declarations a dynamic list's resolvable-field universe is built from.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct MetadataFieldDeclarationIndex {
    tables: BTreeMap<String, MetadataTableStandardAttributes>,
    common_attributes: BTreeMap<String, MetadataCommonAttributeContent>,
    /// Every constant the configuration declares, by its own uuid -- the id a
    /// form's use-always record spells.
    constants: BTreeMap<String, MetadataConstantDeclaration>,
    /// Every `Kind.Name` the configuration declares, folded to lower case: the
    /// query language names metadata case-insensitively, so a query naming
    /// `Перечисление.СтатусызаданийТорговымПредставителям` names the declared
    /// `Enum.СтатусыЗаданийТорговымПредставителям`.
    declared_tables: BTreeSet<String>,
}

impl MetadataFieldDeclarationIndex {
    pub(super) fn table(&self, reference: &str) -> Option<&MetadataTableStandardAttributes> {
        self.tables.get(reference)
    }

    pub(super) fn common_attribute(&self, name: &str) -> Option<&MetadataCommonAttributeContent> {
        self.common_attributes.get(name)
    }

    /// What the constant with this uuid declares, or `None` when this index
    /// carries no constant declarations at all -- a refusal to answer, not a
    /// denial, so a context built without them keeps writing exactly the
    /// record.
    pub(super) fn constant(&self, uuid: &str) -> Option<&MetadataConstantDeclaration> {
        self.constants.get(uuid)
    }

    /// The constants this configuration declares always used, by uuid. Empty
    /// when the index carries no constant declarations.
    pub(super) fn always_used_constants(&self) -> impl Iterator<Item = &str> {
        self.constants
            .iter()
            .filter(|(_, declared)| declared.always_used)
            .map(|(uuid, _)| uuid.as_str())
    }

    /// Whether the configuration declares a metadata table by this
    /// `Kind.Name`. `None` when this index carries no table names at all, which
    /// is a refusal to answer rather than a denial.
    pub(super) fn declares_table(&self, reference: &str) -> Option<bool> {
        if self.declared_tables.is_empty() {
            return None;
        }
        Some(self.declared_tables.contains(&reference.to_lowercase()))
    }

    #[cfg(test)]
    pub(super) fn with_table(
        mut self,
        reference: &str,
        declared: MetadataTableStandardAttributes,
    ) -> Self {
        self.tables.insert(reference.to_string(), declared);
        self
    }

    #[cfg(test)]
    pub(super) fn with_constant(
        mut self,
        uuid: &str,
        declared: MetadataConstantDeclaration,
    ) -> Self {
        self.constants.insert(uuid.to_string(), declared);
        self
    }

    #[cfg(test)]
    pub(super) fn with_common_attribute(
        mut self,
        name: &str,
        content: MetadataCommonAttributeContent,
    ) -> Self {
        self.common_attributes.insert(name.to_string(), content);
        self
    }
}

#[cfg(test)]
impl MetadataTableStandardAttributes {
    pub(super) fn with_code_length(mut self, code_length: u32) -> Self {
        self.code_length = Some(code_length);
        self
    }

    pub(super) fn with_owners(mut self, owned: bool) -> Self {
        self.owned = Some(owned);
        self
    }
}

#[cfg(test)]
impl MetadataCommonAttributeContent {
    pub(super) fn declared(auto_use: bool, content: &[(&str, bool)]) -> Self {
        Self {
            auto_use,
            content: content
                .iter()
                .map(|(table, used)| ((*table).to_string(), *used))
                .collect(),
        }
    }
}

/// Reads the declarations above off the same metadata records the rest of the
/// dump is written from.
pub(super) fn build_metadata_field_declaration_index_from_texts(
    rows: &[MetadataTextRow],
    object_refs: &BTreeMap<String, String>,
) -> MetadataFieldDeclarationIndex {
    let mut index = MetadataFieldDeclarationIndex::default();
    index.declared_tables = object_refs
        .values()
        .filter(|reference| reference.split('.').count() == 2)
        .map(|reference| reference.to_lowercase())
        .collect();
    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        match kind {
            "Catalog" => {
                if let Some(declared) = catalog_declared_standard_attributes(&row.text, header) {
                    index
                        .tables
                        .insert(format!("Catalog.{}", header.name), declared);
                }
            }
            "InformationRegister" => {
                if let Some(declared) =
                    information_register_declared_standard_attributes(&row.text, header)
                {
                    index
                        .tables
                        .insert(format!("InformationRegister.{}", header.name), declared);
                }
            }
            "CommonAttribute" => {
                if let Some(content) = common_attribute_declared_content(&row.text, object_refs) {
                    index.common_attributes.insert(header.name.clone(), content);
                }
            }
            "Constant" => {
                if let Some(declared) = constant_declared_use_always(&row.text, &header.uuid) {
                    index.constants.insert(header.uuid.clone(), declared);
                }
            }
            _ => {}
        }
    }
    if let Ok(path) = std::env::var("IBCMD_UA_CONST_PROBE") {
        let mut dump = String::new();
        for (uuid, declared) in &index.constants {
            dump.push_str(&format!(
                "{uuid}\t{}\t{}\n",
                declared.always_used, declared.value_storage
            ));
        }
        let _ = std::fs::write(path, dump);
    }
    index
}

/// Catalog properties read off the very slots
/// `parse_strict_catalog_properties_from_text` reads for the catalog's own
/// `Catalogs/<name>.xml`.
fn catalog_declared_standard_attributes(
    text: &str,
    header: &MetadataHeader,
) -> Option<MetadataTableStandardAttributes> {
    let mut diagnostic = None;
    let graph = decode_owner_graph_for_family_parser(
        owner_graph::OwnerGraphFamily::Catalog,
        text,
        header,
        &mut diagnostic,
    )?;
    let fields = &graph.owner_fields;
    Some(MetadataTableStandardAttributes {
        code_length: parse_exchange_plan_u32(fields.get(CATALOG_OWNER_FIELD_CODE_LENGTH)?),
        description_length: parse_exchange_plan_u32(
            fields.get(CATALOG_OWNER_FIELD_DESCRIPTION_LENGTH)?,
        ),
        owned: metadata_reference_collection_len(fields.get(CATALOG_OWNER_FIELD_OWNERS)?)
            .map(|count| count > 0),
        hierarchical: information_register_bool(fields.get(CATALOG_OWNER_FIELD_HIERARCHICAL)?),
        folder_hierarchy: catalog_hierarchy_type_xml(
            fields.get(CATALOG_OWNER_FIELD_HIERARCHY_TYPE)?,
        )
        .map(|hierarchy_type| hierarchy_type == "HierarchyFoldersAndItems"),
        periodical: None,
    })
}

/// The register's `<InformationRegisterPeriodicity>`, off the slot
/// `parse_information_register_owner_properties` reads for the register's own
/// `InformationRegisters/<name>.xml`.
fn information_register_declared_standard_attributes(
    text: &str,
    header: &MetadataHeader,
) -> Option<MetadataTableStandardAttributes> {
    let fields = parse_information_register_owner_fields(text, header)?;
    let periodicity = information_register_periodicity_xml(
        fields.get(INFORMATION_REGISTER_OWNER_FIELD_PERIODICITY)?,
    )?;
    Some(MetadataTableStandardAttributes {
        periodical: Some(periodicity != "Nonperiodical"),
        ..MetadataTableStandardAttributes::default()
    })
}

/// The platform's own `v8:ValueStorage`, the type id
/// `metadata_builtin_type_reference` names.
const VALUE_STORAGE_TYPE_UUID: &str = "e199ca70-93cf-46ce-a54b-6edc88c3a296";

/// The constant's own always-used flag and value type, off the same `{16,…}`
/// owner record `parse_constant_properties_from_text` reads every exported
/// constant property from -- slot 11 there, between the default form of slot 10
/// and the data history of slot 12.
fn constant_declared_use_always(text: &str, uuid: &str) -> Option<MetadataConstantDeclaration> {
    let marker = format!("{{1,0,{uuid}}}");
    let marker_start = text.find(&marker)?;
    let owner_start = text[..marker_start].rfind("{16,")?;
    let owner_fields = split_1c_braced_fields(text, owner_start)?;
    let always_used = match owner_fields.get(11)?.trim() {
        "0" => false,
        "1" => true,
        // A flag this reader cannot name is a refusal: the constant is left out
        // of the index and its use-always record is written exactly as stored.
        _ => return None,
    };
    let detail_fields = split_1c_braced_fields(owner_fields.get(1)?, 0)?;
    let value_storage = constant_declared_pattern_type_uuid(detail_fields.get(1).copied())
        .is_some_and(|type_uuid| type_uuid == VALUE_STORAGE_TYPE_UUID);
    Some(MetadataConstantDeclaration {
        always_used,
        value_storage,
    })
}

/// The single platform type id a constant's `{"Pattern",{"#",<uuid>}}` names,
/// when it names exactly one.
fn constant_declared_pattern_type_uuid(detail: Option<&str>) -> Option<String> {
    let fields = split_1c_braced_fields(detail?.trim(), 0)?;
    let pattern = split_1c_braced_fields(fields.get(2)?.trim(), 0)?;
    if pattern.first().map(|value| value.trim()) != Some(r#""Pattern""#) || pattern.len() != 2 {
        return None;
    }
    let entry = split_1c_braced_fields(pattern.get(1)?.trim(), 0)?;
    if entry.first().map(|value| value.trim()) != Some("\"#\"") || entry.len() != 2 {
        return None;
    }
    parse_uuid_field(entry.get(1)?.trim())
}

/// `<AutoUse>` and `<Content>` off the same record
/// `parse_common_attribute_properties_from_text` writes them from.
fn common_attribute_declared_content(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataCommonAttributeContent> {
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|field| field.trim()) != Some("5") {
        return None;
    }
    let use_fields = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0));
    let auto_use = match parse_common_attribute_declared_auto_use(&fields, use_fields.as_deref())? {
        "Use" => true,
        "DontUse" => false,
        // `AutoUse` itself is unobserved on the stand and its meaning for a
        // table the content does not name is unevidenced: refused, not guessed.
        _ => return None,
    };
    let content = parse_common_attribute_content(use_fields.as_deref()?, object_refs)
        .into_iter()
        .map(|item| (item.metadata, item.use_mode == "Use"))
        .collect();
    Some(MetadataCommonAttributeContent { auto_use, content })
}

#[allow(dead_code)]
pub(super) fn build_metadata_object_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, String> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_metadata_object_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_metadata_object_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, String> {
    build_metadata_object_reference_indexes_from_texts(rows).references
}

pub(super) fn build_metadata_object_reference_indexes_from_texts(
    rows: &[MetadataTextRow],
) -> MetadataObjectReferenceIndexes {
    let mut index = MetadataObjectReferenceIndexes::default();
    let empty_form_refs = BTreeMap::new();
    let empty_template_refs = BTreeMap::new();
    let subsystem_refs = build_subsystem_source_reference_index_from_texts(rows);
    let recalculation_refs = build_calculation_recalculation_reference_index(rows);
    for row in rows {
        if let Some(name) = parse_configuration_reference_text_for_row(&row.text, &row.file_name) {
            index.insert(row.file_name.clone(), format!("Configuration.{name}"));
            continue;
        }
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        let reference = if kind == "Subsystem" {
            subsystem_refs
                .get(&header.uuid)
                .and_then(subsystem_source_reference_name)
                .unwrap_or_else(|| format!("{kind}.{}", header.name))
        } else {
            format!("{kind}.{}", header.name)
        };
        index.insert(row.file_name.clone(), reference);
        if kind == "Enum" {
            for value in parse_enum_values_from_text(&row.text) {
                index.insert(
                    value.uuid,
                    format!("Enum.{}.EnumValue.{}", header.name, value.name),
                );
            }
        }
        for command in nested_command_headers_for_owner_from_text(kind, &row.text, &row.file_name) {
            index.insert(
                command.uuid,
                format!("{}.{}.Command.{}", kind, header.name, command.name),
            );
        }
        for (child, marker_start) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                &empty_form_refs,
                &empty_template_refs,
            ) {
                index.or_insert(child.uuid, reference);
            }
        }
        if kind == "WebService" {
            let operations =
                nested_web_service_operation_headers_from_text(&row.text, &row.file_name);
            for operation in &operations {
                index.insert(
                    operation.uuid.clone(),
                    format!("WebService.{}.Operation.{}", header.name, operation.name),
                );
            }
            insert_web_service_parameter_refs(
                &mut index,
                &row.text,
                &row.file_name,
                &header.name,
                &operations,
            );
        }
        if kind == "HTTPService" {
            insert_http_service_child_role_refs(&mut index, &row.text, &header.uuid, &header.name);
        }
    }
    for (uuid, recalculation) in &recalculation_refs {
        index.insert(uuid.clone(), recalculation.object_reference());
    }
    insert_recalculation_dimension_refs(&mut index, rows, &recalculation_refs);
    index
}

pub(super) fn build_configuration_root_object_reference_index_from_texts(
    rows: &[MetadataTextRow],
    object_refs: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut index = object_refs.clone();
    for row in rows {
        if row.object_code != Some(0) || !is_defined_type_metadata_text(&row.text, &row.file_name) {
            continue;
        }
        let Some(header) = row.header.as_ref() else {
            continue;
        };
        index.insert(
            row.file_name.clone(),
            format!("DefinedType.{}", header.name),
        );
    }
    index
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CalculationRecalculationReference {
    pub(super) owner_name: String,
    pub(super) recalculation_name: String,
}

impl CalculationRecalculationReference {
    pub(super) fn object_reference(&self) -> String {
        format!(
            "CalculationRegister.{}.Recalculation.{}",
            self.owner_name, self.recalculation_name
        )
    }
}

pub(super) fn build_calculation_recalculation_reference_index(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, CalculationRecalculationReference> {
    let headers_by_uuid = metadata_headers_by_uuid(rows);
    let mut refs = BTreeMap::new();
    let mut owner_uuids = BTreeMap::<String, String>::new();
    let mut ambiguous = BTreeSet::new();
    for row in rows {
        let (Some("CalculationRegister"), Some(owner)) = (row.kind.as_deref(), row.header.as_ref())
        else {
            continue;
        };
        let declared = calculation_register_recalculation_uuids_from_text(&row.text);
        for uuid in declared {
            let Some(recalculation) = headers_by_uuid.get(&uuid) else {
                continue;
            };
            let reference = CalculationRecalculationReference {
                owner_name: owner.name.clone(),
                recalculation_name: recalculation.name.clone(),
            };
            if let Some(previous) = refs.get(&uuid) {
                if previous != &reference
                    || owner_uuids.get(&uuid).map(String::as_str) != Some(owner.uuid.as_str())
                {
                    refs.remove(&uuid);
                    owner_uuids.remove(&uuid);
                    ambiguous.insert(uuid);
                }
            } else if !ambiguous.contains(&uuid) {
                owner_uuids.insert(uuid.clone(), owner.uuid.clone());
                refs.insert(uuid, reference);
            }
        }
    }
    let mut ids_by_path = BTreeMap::<(String, String), String>::new();
    let mut colliding_ids = BTreeSet::new();
    for (uuid, reference) in &refs {
        let path_key = (
            sanitize_source_path_segment(&reference.owner_name),
            sanitize_source_path_segment(&reference.recalculation_name),
        );
        if let Some(previous_uuid) = ids_by_path.insert(path_key, uuid.clone()) {
            colliding_ids.insert(previous_uuid);
            colliding_ids.insert(uuid.clone());
        }
    }
    for uuid in colliding_ids {
        refs.remove(&uuid);
    }
    refs
}

fn insert_web_service_parameter_refs(
    index: &mut MetadataObjectReferenceIndexes,
    text: &str,
    owner_uuid: &str,
    owner_name: &str,
    operations: &[MetadataHeader],
) {
    let parameter_list_marker = format!("{{{WEB_SERVICE_PARAMETER_COLLECTION_UUID},");

    let operation_ids = operations
        .iter()
        .map(|operation| operation.uuid.as_str())
        .collect::<BTreeSet<_>>();
    let nested = nested_headers_with_offsets_from_text(text, owner_uuid, |_| true);
    let operation_offsets = nested
        .iter()
        .filter(|(header, _)| operation_ids.contains(header.uuid.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    let mut offset = 0usize;
    while let Some(relative_start) = text[offset..].find(&parameter_list_marker) {
        let start = offset + relative_start;
        offset = start + parameter_list_marker.len();
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        let Some((operation, _)) = operation_offsets
            .iter()
            .rev()
            .find(|(_, operation_start)| *operation_start < start)
        else {
            continue;
        };
        let operation_ref = format!("WebService.{owner_name}.Operation.{}", operation.name);
        for (parameter, parameter_start) in &nested {
            if *parameter_start <= start
                || *parameter_start >= end
                || operation_ids.contains(parameter.uuid.as_str())
            {
                continue;
            }
            index.insert(
                parameter.uuid.clone(),
                format!("{operation_ref}.Parameter.{}", parameter.name),
            );
        }
    }
}

fn insert_recalculation_dimension_refs(
    index: &mut MetadataObjectReferenceIndexes,
    rows: &[MetadataTextRow],
    recalculation_refs: &BTreeMap<String, CalculationRecalculationReference>,
) {
    for row in rows {
        let Some(recalculation) = recalculation_refs.get(&row.file_name) else {
            continue;
        };
        let owner_ref = recalculation.object_reference();
        for (dimension, _marker_start) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |marker_start| {
                is_offset_inside_recalculation_dimension_list(&row.text, marker_start)
            })
        {
            index.insert(
                dimension.uuid,
                format!("{owner_ref}.Dimension.{}", dimension.name),
            );
        }
    }
}

fn metadata_headers_by_uuid(rows: &[MetadataTextRow]) -> BTreeMap<String, MetadataHeader> {
    rows.iter()
        .filter_map(|row| {
            row.header
                .as_ref()
                .map(|header| (header.uuid.clone(), header.clone()))
        })
        .collect()
}

pub(super) fn build_role_rights_object_reference_index(
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> BTreeMap<String, String> {
    let mut refs = object_refs.clone();
    for (uuid, form_ref) in form_refs {
        if let Some(reference) = form_source_reference_name(form_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    refs
}

pub(super) fn build_metadata_order_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    for row in rows {
        let Some(header_uuid) = parse_configuration_header_uuid(&row.text) else {
            continue;
        };
        for (order, child) in
            parse_configuration_child_objects(&row.text, &row.file_name, &header_uuid)
                .into_iter()
                .enumerate()
        {
            index.entry(child.header.uuid).or_insert(order);
        }
    }
    index
}

/// Returns the exact top-level metadata UUID inventory embedded in the
/// Configuration root row. Unlike the broad reference indexes, this list does
/// not contain forms, templates, or other dependency rows owned by a selected
/// object.
pub(super) fn configuration_root_metadata_file_names_from_texts(
    rows: &[MetadataTextRow],
) -> Option<BTreeSet<String>> {
    let mut roots = None::<BTreeSet<String>>;
    for row in rows {
        let Some(layout) = parse_configuration_root_layout(&row.text, &row.file_name) else {
            continue;
        };
        if roots.is_some() {
            // A Config table has one Configuration root. Refuse to turn an
            // ambiguous index into evidence for a complete export.
            return None;
        }
        let mut current = BTreeSet::from([row.file_name.clone()]);
        current.extend(
            layout
                .child_families
                .iter()
                .flat_map(|family| family.iter().cloned()),
        );
        roots = Some(current);
    }
    roots
}

pub(super) fn parse_configuration_header_uuid(text: &str) -> Option<String> {
    if !text.trim_start().starts_with("{2,") {
        return None;
    }
    let marker = "{1,0,";
    let marker_start = text.find(marker)?;
    let header_uuid_start = marker_start + marker.len();
    let header_uuid_end = header_uuid_start + 36;
    let header_uuid = text.get(header_uuid_start..header_uuid_end)?;
    if !is_uuid_text(header_uuid) || !is_metadata_header_marker(text, header_uuid_end) {
        return None;
    }
    Some(header_uuid.to_string())
}

pub(super) fn insert_http_service_child_role_refs(
    index: &mut MetadataObjectReferenceIndexes,
    text: &str,
    owner_uuid: &str,
    owner_name: &str,
) {
    for template in parse_http_service_url_templates_from_text(text, owner_uuid) {
        let template_ref = format!(
            "HTTPService.{owner_name}.URLTemplate.{}",
            template.header.name
        );
        index.insert(template.header.uuid.clone(), template_ref.clone());
        for method in template.methods {
            index.insert(
                method.header.uuid,
                format!("{template_ref}.Method.{}", method.header.name),
            );
        }
    }
}

pub(super) fn build_standalone_content_references(
    rows: &[MetadataTextRow],
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> StandaloneContentReferences {
    let mut standalone_object_refs = object_refs.clone();
    for (uuid, form_ref) in form_refs {
        if let Some(reference) = form_source_reference_name(form_ref) {
            standalone_object_refs.insert(uuid.clone(), reference);
        }
    }
    for (uuid, template_ref) in template_refs {
        if let Some(reference) = template_source_reference_name(template_ref) {
            standalone_object_refs.insert(uuid.clone(), reference);
        }
    }
    for (uuid, subsystem_ref) in subsystem_refs {
        if let Some(reference) = subsystem_source_reference_name(subsystem_ref) {
            standalone_object_refs.insert(uuid.clone(), reference);
        }
    }

    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        let mut seen = BTreeSet::new();
        for (child, marker_start) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                form_refs,
                template_refs,
            ) && seen.insert(child.uuid.clone())
            {
                standalone_object_refs.insert(child.uuid, reference);
            }
        }
        for uuid in uuid_like_values(&row.text) {
            if standalone_object_refs.contains_key(&uuid) {
                continue;
            }
            if let Some(reference) = form_refs.get(&uuid).and_then(form_source_reference_name) {
                standalone_object_refs.insert(uuid, reference);
            } else if let Some(reference) = template_refs
                .get(&uuid)
                .and_then(template_source_reference_name)
            {
                standalone_object_refs.insert(uuid, reference);
            }
        }
    }

    StandaloneContentReferences {
        object_refs: standalone_object_refs,
    }
}

pub(super) fn build_standalone_object_reference_index_from_texts(
    rows: &[MetadataTextRow],
    required_refs: &BTreeSet<String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    if required_refs.is_empty() {
        return index;
    }

    for row in rows {
        if required_refs.contains(&row.file_name) {
            if let Some(name) =
                parse_configuration_reference_text_for_row(&row.text, &row.file_name)
            {
                index.insert(row.file_name.clone(), format!("Configuration.{name}"));
                continue;
            }
            let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
                continue;
            };
            let reference = if kind == "Subsystem" {
                subsystem_refs
                    .get(&header.uuid)
                    .and_then(subsystem_source_reference_name)
                    .unwrap_or_else(|| format!("{kind}.{}", header.name))
            } else {
                format!("{kind}.{}", header.name)
            };
            index.insert(row.file_name.clone(), reference);
        }

        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        if kind == "Enum" {
            for value in parse_enum_values_from_text(&row.text) {
                if required_refs.contains(&value.uuid) {
                    index.insert(
                        value.uuid,
                        format!("Enum.{}.EnumValue.{}", header.name, value.name),
                    );
                }
            }
        }
        if kind == "HTTPService" {
            insert_required_http_service_child_role_refs(
                &mut index,
                &row.text,
                &header.uuid,
                &header.name,
                required_refs,
            );
        }
        for (child, marker_start) in
            nested_headers_with_offsets_matching_uuids(&row.text, &row.file_name, required_refs)
        {
            if index.contains_key(&child.uuid) {
                continue;
            }
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                form_refs,
                template_refs,
            ) {
                index.insert(child.uuid, reference);
            }
        }
    }

    index
}

pub(super) fn insert_required_http_service_child_role_refs(
    index: &mut BTreeMap<String, String>,
    text: &str,
    owner_uuid: &str,
    owner_name: &str,
    required_refs: &BTreeSet<String>,
) {
    for template in parse_http_service_url_templates_from_text(text, owner_uuid) {
        let template_ref = format!(
            "HTTPService.{owner_name}.URLTemplate.{}",
            template.header.name
        );
        if required_refs.contains(&template.header.uuid) {
            index.insert(template.header.uuid.clone(), template_ref.clone());
        }
        for method in template.methods {
            if required_refs.contains(&method.header.uuid) {
                index.insert(
                    method.header.uuid,
                    format!("{template_ref}.Method.{}", method.header.name),
                );
            }
        }
    }
}

pub(super) fn build_standalone_content_references_for_uuids(
    rows: &[MetadataTextRow],
    required_refs: &BTreeSet<String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> StandaloneContentReferences {
    let mut standalone_object_refs = object_refs.clone();
    for uuid in required_refs {
        if standalone_object_refs.contains_key(uuid) {
            continue;
        }
        if let Some(reference) = form_refs.get(uuid).and_then(form_source_reference_name) {
            standalone_object_refs.insert(uuid.clone(), reference);
        } else if let Some(reference) = template_refs
            .get(uuid)
            .and_then(template_source_reference_name)
        {
            standalone_object_refs.insert(uuid.clone(), reference);
        } else if let Some(reference) = subsystem_refs
            .get(uuid)
            .and_then(subsystem_source_reference_name)
        {
            standalone_object_refs.insert(uuid.clone(), reference);
        }
    }

    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        for (child, marker_start) in
            nested_headers_with_offsets_matching_uuids(&row.text, &row.file_name, required_refs)
        {
            if standalone_object_refs.contains_key(&child.uuid) {
                continue;
            }
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                form_refs,
                template_refs,
            ) {
                standalone_object_refs.insert(child.uuid, reference);
            }
        }
    }

    StandaloneContentReferences {
        object_refs: standalone_object_refs,
    }
}

pub(super) fn build_help_reference_index(
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, String> {
    let mut refs = object_refs.clone();
    for (uuid, form_ref) in form_refs {
        if let Some(reference) = form_source_reference_name(form_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    for (uuid, template_ref) in template_refs {
        if let Some(reference) = template_source_reference_name(template_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    for (uuid, subsystem_ref) in subsystem_refs {
        if let Some(reference) = subsystem_source_reference_name(subsystem_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    refs
}

pub(super) fn build_functional_option_reference_index_from_texts(
    rows: &[MetadataTextRow],
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, String> {
    let mut refs = object_refs.clone();
    for (uuid, subsystem_ref) in subsystem_refs {
        if let Some(reference) = subsystem_source_reference_name(subsystem_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    let required_refs = functional_option_reference_uuids_from_texts(rows);
    if required_refs.is_empty() {
        return refs;
    }
    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        let mut seen = BTreeSet::new();
        for (child, marker_start) in
            nested_headers_with_offsets_matching_uuids(&row.text, &row.file_name, &required_refs)
        {
            if refs.contains_key(&child.uuid) || seen.contains(&child.uuid) {
                continue;
            }
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                form_refs,
                template_refs,
            ) {
                seen.insert(child.uuid.clone());
                refs.insert(child.uuid, reference);
            }
        }
    }
    refs
}

pub(super) fn functional_option_reference_uuids_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for row in rows {
        if row.kind.as_deref() != Some("FunctionalOption") {
            continue;
        }
        let Some(fields) = metadata_object_fields(&row.text) else {
            continue;
        };
        if let Some(uuid) = fields
            .get(2)
            .and_then(|field| parse_non_zero_uuid(field.trim()))
        {
            refs.insert(uuid);
        }
        if let Some(content) = fields.get(3) {
            refs.extend(uuid_like_values_in_text_order(content));
        }
    }
    refs
}

pub(super) fn nested_headers_with_offsets_matching_uuids(
    text: &str,
    owner_uuid: &str,
    uuids: &BTreeSet<String>,
) -> Vec<(MetadataHeader, usize)> {
    let mut headers = Vec::new();
    let mut seen = BTreeSet::new();
    let mut offset = 0usize;
    let marker = "{1,0,";

    while let Some(relative) = text[offset..].find(marker) {
        let marker_start = offset + relative;
        let uuid_start = marker_start + marker.len();
        let uuid_end = uuid_start + 36;
        offset = uuid_start;

        let Some(uuid) = text.get(uuid_start..uuid_end) else {
            continue;
        };
        if uuid == owner_uuid
            || !uuids.contains(uuid)
            || !is_uuid_text(uuid)
            || !is_metadata_header_marker(text, uuid_end)
            || !seen.insert(uuid.to_string())
        {
            continue;
        }
        if let Some(header) = parse_metadata_header_from_text(text, uuid) {
            headers.push((header, marker_start));
        }
    }

    headers
}

pub(super) fn standalone_child_reference(
    owner_kind: &str,
    owner_name: &str,
    owner_uuid: &str,
    text: &str,
    marker_start: usize,
    child: &MetadataHeader,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<String> {
    if let Some(reference) = form_refs
        .get(&child.uuid)
        .and_then(form_source_reference_name)
    {
        return Some(reference);
    }
    if let Some(reference) = template_refs
        .get(&child.uuid)
        .and_then(template_source_reference_name)
    {
        return Some(reference);
    }
    if owner_kind == "InformationRegister"
        && let Some(tag) = register_child_object_tag(owner_kind, text, marker_start)
    {
        return Some(format!("{owner_kind}.{owner_name}.{tag}.{}", child.name));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 9) {
        return Some(format!("{owner_kind}.{owner_name}.Command.{}", child.name));
    }
    if owner_kind == "WebService" && is_offset_inside_metadata_object_code(text, marker_start, 1) {
        return Some(format!("WebService.{owner_name}.Operation.{}", child.name));
    }
    if owner_kind == "IntegrationService"
        && is_offset_inside_metadata_object_code(text, marker_start, 1)
    {
        return Some(format!(
            "IntegrationService.{owner_name}.IntegrationServiceChannel.{}",
            child.name
        ));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 11) {
        if let Some(tabular_section) = enclosing_metadata_header_for_code(text, marker_start, 11)
            && tabular_section.uuid != child.uuid
        {
            return Some(format!(
                "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
                tabular_section.name, child.name
            ));
        }
        return Some(format!(
            "{owner_kind}.{owner_name}.TabularSection.{}",
            child.name
        ));
    }
    if let Some(tabular_section) = enclosing_metadata_header_for_code(text, marker_start, 11)
        && tabular_section.uuid != child.uuid
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
            tabular_section.name, child.name
        ));
    }
    if let Some(reference) = tabular_section_attribute_reference(
        owner_kind,
        owner_name,
        owner_uuid,
        text,
        marker_start,
        child,
    ) {
        return Some(reference);
    }
    if owner_kind == "AccountingRegister"
        && is_offset_inside_register_dimension_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 6)
    {
        return Some(format!(
            "AccountingRegister.{owner_name}.Dimension.{}",
            child.name
        ));
    }
    if owner_kind == "AccountingRegister"
        && is_offset_inside_register_resource_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 2)
    {
        return Some(format!(
            "AccountingRegister.{owner_name}.Resource.{}",
            child.name
        ));
    }
    if owner_kind == "AccountingRegister"
        && is_offset_inside_accounting_register_attribute_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 2)
    {
        return Some(format!(
            "AccountingRegister.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if owner_kind == "AccumulationRegister"
        && is_offset_inside_accumulation_register_attribute_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 2)
    {
        // Same shape as the AccountingRegister arm above, and all 74
        // measured attribute headers satisfy the code-2 containment too
        // (see `is_offset_inside_accumulation_register_attribute_list`).
        return Some(format!(
            "AccumulationRegister.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if owner_kind == "CalculationRegister"
        && is_offset_inside_calculation_register_attribute_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 2)
    {
        return Some(format!(
            "CalculationRegister.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if owner_kind == "ChartOfCalculationTypes"
        && is_offset_inside_chart_of_calculation_types_attribute_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 2)
    {
        return Some(format!(
            "ChartOfCalculationTypes.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if owner_kind == "ChartOfAccounts"
        && is_offset_inside_chart_of_accounts_accounting_flag_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 6)
    {
        return Some(format!(
            "ChartOfAccounts.{owner_name}.AccountingFlag.{}",
            child.name
        ));
    }
    if owner_kind == "ChartOfAccounts"
        && is_offset_inside_chart_of_accounts_ext_dimension_accounting_flag_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 6)
    {
        return Some(format!(
            "ChartOfAccounts.{owner_name}.ExtDimensionAccountingFlag.{}",
            child.name
        ));
    }
    if metadata_kind_uses_register_resources(owner_kind)
        && is_offset_inside_register_resource_list(text, marker_start)
    {
        return Some(format!("{owner_kind}.{owner_name}.Resource.{}", child.name));
    }
    if metadata_kind_uses_register_resources(owner_kind)
        && is_offset_inside_register_dimension_list(text, marker_start)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Dimension.{}",
            child.name
        ));
    }
    if owner_kind == "Sequence" && is_offset_inside_sequence_dimension_list(text, marker_start) {
        return Some(format!("Sequence.{owner_name}.Dimension.{}", child.name));
    }
    if owner_kind == "CalculationRegister"
        && is_offset_inside_metadata_object_code(text, marker_start, 4)
        && is_offset_inside_calculation_register_recalculation_list(text, marker_start)
    {
        return Some(format!(
            "CalculationRegister.{owner_name}.Recalculation.{}",
            child.name
        ));
    }
    if owner_kind == "Task"
        && is_offset_inside_metadata_object_code(text, marker_start, 4)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(format!(
            "Task.{owner_name}.AddressingAttribute.{}",
            child.name
        ));
    }
    if owner_kind == "DataProcessor"
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
        && is_offset_inside_tabular_section_attribute_list(text, marker_start)
        && let Some((tabular_section, tabular_end)) =
            preceding_metadata_header_for_code_with_bounds(text, marker_start, 11)
        && tabular_section.uuid != child.uuid
        && !contains_metadata_header_uuid_between(text, tabular_end, marker_start, owner_uuid)
        && !contains_metadata_header_name_between(text, tabular_end, marker_start, owner_name)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
            tabular_section.name, child.name
        ));
    }
    if owner_kind == "DataProcessor"
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if metadata_kind_uses_code27_attributes(owner_kind)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if metadata_kind_uses_code4_attributes(owner_kind)
        && is_offset_inside_metadata_object_code(text, marker_start, 4)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if owner_kind == "BusinessProcess"
        && is_offset_inside_metadata_object_code(text, marker_start, 3)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
        && !is_offset_inside_metadata_object_code(text, marker_start, 8)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 5) {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 6) {
        if let Some(tabular_section) = enclosing_metadata_header_for_code(text, marker_start, 11)
            && tabular_section.uuid != child.uuid
        {
            return Some(format!(
                "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
                tabular_section.name, child.name
            ));
        }
        if is_offset_inside_tabular_section_attribute_list(text, marker_start)
            && let Some(tabular_section) =
                preceding_metadata_header_for_code(text, marker_start, 11)
            && tabular_section.uuid != child.uuid
        {
            return Some(format!(
                "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
                tabular_section.name, child.name
            ));
        }
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 8) {
        if let Some(tabular_section) = preceding_metadata_header_for_code(text, marker_start, 11) {
            return Some(format!(
                "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
                tabular_section.name, child.name
            ));
        }
        return Some(format!("{owner_kind}.{owner_name}.Resource.{}", child.name));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 10) {
        return Some(format!(
            "{owner_kind}.{owner_name}.Dimension.{}",
            child.name
        ));
    }
    None
}

pub(super) fn tabular_section_attribute_reference(
    owner_kind: &str,
    owner_name: &str,
    _owner_uuid: &str,
    text: &str,
    marker_start: usize,
    child: &MetadataHeader,
) -> Option<String> {
    if !is_offset_inside_tabular_section_attribute_list(text, marker_start) {
        return None;
    }
    let (tabular_section, _) =
        preceding_metadata_header_for_code_with_bounds(text, marker_start, 11)?;
    if tabular_section.uuid == child.uuid {
        return None;
    }
    Some(format!(
        "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
        tabular_section.name, child.name
    ))
}

pub(super) fn metadata_kind_uses_register_resources(kind: &str) -> bool {
    matches!(
        kind,
        "AccumulationRegister"
            | "AccountingRegister"
            | "CalculationRegister"
            | "InformationRegister"
    )
}

pub(super) fn metadata_kind_uses_code27_attributes(kind: &str) -> bool {
    matches!(
        kind,
        "ChartOfAccounts" | "ChartOfCharacteristicTypes" | "ExchangePlan" | "Report" | "Task"
    )
}

pub(super) fn metadata_kind_uses_code4_attributes(kind: &str) -> bool {
    kind == "ExchangePlan" || metadata_kind_uses_register_resources(kind)
}

pub(super) fn is_offset_inside_register_resource_list(text: &str, offset: usize) -> bool {
    is_offset_inside_any_list_marker(
        text,
        offset,
        &[
            "{b64d9a41-1642-11d6-a3c7-0050bae0a776,",
            "{63405499-7491-4ce3-ac72-43433cbe4112,",
            "{702b33ad-843e-41aa-8064-112cd38cc92c,",
        ],
    )
}

pub(super) fn is_offset_inside_register_dimension_list(text: &str, offset: usize) -> bool {
    is_offset_inside_any_list_marker(
        text,
        offset,
        &[
            "{b64d9a43-1642-11d6-a3c7-0050bae0a776,",
            "{35b63b9d-0adf-4625-a047-10ae874c19a3,",
            "{b12fc850-8210-43c8-ae05-89567e698fbb,",
        ],
    )
}

pub(super) fn is_offset_inside_accounting_register_attribute_list(
    text: &str,
    offset: usize,
) -> bool {
    is_offset_inside_any_list_marker(text, offset, &["{9d28ee33-9c7e-4a1b-8f13-50aa9b36607b,"])
}

/// True inside an accumulation register's attribute list.
///
/// The register's three child families are adjacent UUIDs: `…a41` resources,
/// `…a42` attributes, `…a43` dimensions (the latter two already named by
/// `is_offset_inside_register_resource_list` /
/// `is_offset_inside_register_dimension_list`). Measured on ERP УХ 3.2.12.6
/// (2026-08-25) over nine extracted `AccumulationRegisters/*` storage
/// elements: the headers inside `…a42` spans are exactly the 74 `Attribute`
/// uuids their native XML declares -- none missing, and no `Dimension` or
/// `Resource` header inside any such span.
/// Every collection an owner's commands are serialized into, one family uuid
/// per owner kind.
///
/// Measured on ERP УХ 3.2.12.6 (2026-08-25) by dumping, for every nested
/// header of every metadata row, the innermost enclosing `{uuid,…}` span and
/// cross-referencing the header against the native source tree's own
/// inventory of declared children (139,868 of them, 1,097 `<Command>`):
///
/// * all **1,097/1,097** declared commands lie inside one of these fourteen
///   spans, none outside;
/// * the spans contain **nothing else** -- 0 `Attribute`, `Resource`,
///   `Dimension`, `TabularSection` or any other declared child, in any of the
///   fourteen;
/// * each family belongs to exactly one owner kind, and its member count is
///   that kind's command count exactly (DataProcessor 426, Catalog 208,
///   Document 207, InformationRegister 115, Report 82, DocumentJournal 19,
///   ExchangePlan 12, BusinessProcess 10, Task 8, ChartOfAccounts 3,
///   FilterCriterion 2, ChartOfCharacteristicTypes 2, AccountingRegister 2,
///   AccumulationRegister 1).
///
/// Re-measured the same way on the other five stand corpora: `mdm` 3/3,
/// `sslbase` 102/102, `ssl` 153/153 declared commands inside these families,
/// none outside, nothing else inside; `ws` and `wms` declare no command at
/// all.
const COMMAND_COLLECTION_LIST_MARKERS: [&str; 14] = [
    "{45556acb-826a-4f73-898a-6025fc9536e1,", // DataProcessor
    "{4fe87c89-9ad4-43f6-9fdb-9dc83b3879c6,", // Catalog
    "{b544fc6a-2ba3-4885-8fb2-cb289fb6d65e,", // Document
    "{b44ba719-945c-445c-8aab-1088fa4df16e,", // InformationRegister
    "{e7ff38c0-ec3c-47a0-ae90-20c73ca72246,", // Report
    "{a49a35ce-120a-4c80-8eea-b0618479cd70,", // DocumentJournal
    "{d5207c64-11d5-4d46-bba2-55b7b07ff4eb,", // ExchangePlan
    "{7a3e533c-f232-40d5-a932-6a311d2480bf,", // BusinessProcess
    "{f27c2152-a2c9-4c30-adb1-130f5eb2590f,", // Task
    "{0df30176-6865-4787-9fc8-609eb144174f,", // ChartOfAccounts
    "{23fa3b84-220a-40e9-8331-e588bed87f7d,", // FilterCriterion
    "{95b5e1d4-abfa-4a16-818d-a5b07b7d3f73,", // ChartOfCharacteristicTypes
    "{7162da60-f7fe-4d78-ad5d-e31700f9af18,", // AccountingRegister
    "{99f328af-a77f-4572-a2d8-80ed20c81890,", // AccumulationRegister
];

/// True inside any owner's command collection.
pub(super) fn is_offset_inside_command_collection(text: &str, offset: usize) -> bool {
    is_offset_inside_any_list_marker(text, offset, &COMMAND_COLLECTION_LIST_MARKERS)
}

pub(super) fn is_offset_inside_accumulation_register_attribute_list(
    text: &str,
    offset: usize,
) -> bool {
    is_offset_inside_any_list_marker(text, offset, &["{b64d9a42-1642-11d6-a3c7-0050bae0a776,"])
}

/// True inside a calculation register's attribute list.
///
/// Measured on ERP УХ 3.2.12.6 (2026-08-25) over both extracted
/// `CalculationRegisters/*` storage elements: the headers inside this family's
/// spans are exactly the 25 and 9 `Attribute` uuids their native XML declares
/// -- none missing, no `Dimension` or `Resource` among them -- and all of them
/// satisfy the code-2 containment too. The kind's attributes were previously
/// read through `metadata_kind_uses_code4_attributes`, which also demands code
/// 4; none of the 34 carries it, so none was indexed.
pub(super) fn is_offset_inside_calculation_register_attribute_list(
    text: &str,
    offset: usize,
) -> bool {
    is_offset_inside_any_list_marker(text, offset, &["{1b304502-2216-440b-960f-60decd04bb5d,"])
}

/// True inside a chart of calculation types' own attribute list.
///
/// Measured the same way on `ChartsOfCalculationTypes/Начисления` and
/// `.../Удержания`: 95 and 33 headers inside this family's spans, all of them
/// `Attribute` and nothing else, all inside code 2. The remaining 20 and 9
/// attributes those charts declare belong to their tabular sections and sit in
/// a different family, which already resolves.
pub(super) fn is_offset_inside_chart_of_calculation_types_attribute_list(
    text: &str,
    offset: usize,
) -> bool {
    is_offset_inside_any_list_marker(text, offset, &["{0dc22ad2-476a-4794-afae-cfa7ed251752,"])
}

fn is_offset_inside_chart_of_accounts_accounting_flag_list(text: &str, offset: usize) -> bool {
    is_offset_inside_any_list_marker(text, offset, &["{78bd1243-c4df-46c3-8138-e147465cb9a4,"])
}

fn is_offset_inside_chart_of_accounts_ext_dimension_accounting_flag_list(
    text: &str,
    offset: usize,
) -> bool {
    is_offset_inside_any_list_marker(text, offset, &["{c70ca527-5042-4cad-a315-dcb4007e32a3,"])
}

fn is_offset_inside_sequence_dimension_list(text: &str, offset: usize) -> bool {
    is_offset_inside_any_list_marker(text, offset, &["{437488c0-35e2-11d6-a3c7-0050bae0a776,"])
}

pub(super) const RECALCULATION_DIMENSION_LIST_MARKER: &str = "3c456b74-4ea5-4b22-a957-e9fad9133b54";

fn is_offset_inside_recalculation_dimension_list(text: &str, offset: usize) -> bool {
    is_offset_inside_any_list_marker(text, offset, &["{3c456b74-4ea5-4b22-a957-e9fad9133b54,"])
}

fn is_offset_inside_any_list_marker(text: &str, offset: usize, markers: &[&str]) -> bool {
    markers.iter().any(|marker| {
        let Some(start) = text[..offset].rfind(marker) else {
            return false;
        };
        scan_1c_braced_value(text, start)
            .map(|end| offset < end)
            .unwrap_or(false)
    })
}

pub(super) fn is_offset_inside_calculation_register_recalculation_list(
    text: &str,
    offset: usize,
) -> bool {
    const RECALCULATION_LIST_MARKER: &str = "{274bf899-db0e-4df6-8ab5-67bf6371ec0b,";
    let Some(start) = text[..offset].rfind(RECALCULATION_LIST_MARKER) else {
        return false;
    };
    scan_1c_braced_value(text, start)
        .map(|end| offset < end)
        .unwrap_or(false)
}

pub(super) fn calculation_register_recalculation_uuids_from_text(text: &str) -> Vec<String> {
    const RECALCULATION_LIST_MARKER: &str = "{274bf899-db0e-4df6-8ab5-67bf6371ec0b,";
    let mut uuids = Vec::new();
    let mut seen = BTreeSet::new();
    let mut offset = 0usize;
    while let Some(relative_start) = text[offset..].find(RECALCULATION_LIST_MARKER) {
        let start = offset + relative_start;
        offset = start + RECALCULATION_LIST_MARKER.len();
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        let Some(fields) = split_1c_braced_fields(&text[start..end], 0) else {
            continue;
        };
        let count = fields
            .get(1)
            .and_then(|field| field.trim().parse::<usize>().ok())
            .unwrap_or(0);
        for uuid in fields
            .iter()
            .skip(2)
            .take(count)
            .filter_map(|field| parse_non_zero_uuid(field.trim()))
        {
            if seen.insert(uuid.clone()) {
                uuids.push(uuid);
            }
        }
    }
    uuids
}

pub(super) fn is_offset_inside_tabular_section_attribute_list(text: &str, offset: usize) -> bool {
    is_offset_inside_any_list_marker(
        text,
        offset,
        &[
            "{5d24a9d1-098e-11d6-b9b8-0050bae0a95d,",
            "{888744e1-b616-11d4-9436-004095e12fc7,",
            "{c339c860-29e2-11d6-a3c7-0050bae0a776,",
        ],
    )
}

pub(super) fn is_offset_inside_data_processor_legacy_attribute_list(
    text: &str,
    offset: usize,
) -> bool {
    const DATA_PROCESSOR_LEGACY_ATTRIBUTE_LIST_MARKER: &str =
        "{ec6bb5e5-b7a8-4d75-bec9-658107a699cf,";
    let Some(start) = text[..offset].rfind(DATA_PROCESSOR_LEGACY_ATTRIBUTE_LIST_MARKER) else {
        return false;
    };
    scan_1c_braced_value(text, start)
        .map(|end| offset < end)
        .unwrap_or(false)
}

pub(super) fn template_source_reference_name(
    template_ref: &TemplateSourceReference,
) -> Option<String> {
    let parts = template_ref
        .relative_path
        .iter()
        .filter_map(|part| part.to_str())
        .collect::<Vec<_>>();
    if parts.len() == 2 && parts.first() == Some(&"CommonTemplates") {
        let template_name = Path::new(parts[1]).file_stem()?.to_str()?;
        return Some(format!("CommonTemplate.{template_name}"));
    }
    if parts.len() == 4 && parts.get(2) == Some(&"Templates") {
        let owner_kind = metadata_kind_for_source_folder(parts[0])?;
        let owner_name = parts[1];
        let template_name = Path::new(parts[3]).file_stem()?.to_str()?;
        return Some(format!(
            "{owner_kind}.{owner_name}.Template.{template_name}"
        ));
    }
    None
}

pub(super) fn subsystem_source_reference_name(
    subsystem_ref: &SubsystemSourceReference,
) -> Option<String> {
    let mut names = Vec::new();
    for part in subsystem_ref
        .relative_path
        .iter()
        .filter_map(|part| part.to_str())
    {
        if part == "Subsystems" {
            continue;
        }
        let name = Path::new(part).file_stem()?.to_str()?;
        names.push(name.to_string());
    }
    let mut names = names.into_iter();
    let first = names.next()?;
    let mut reference = format!("Subsystem.{first}");
    for name in names {
        reference.push_str(".Subsystem.");
        reference.push_str(&name);
    }
    Some(reference)
}

/// `ChildObjects` in a subsystem source document lists child object *names*,
/// exactly like every other metadata family, rather than the qualified
/// `Subsystem.Owner.Subsystem.Child` reference that cross-object properties
/// use. Native 1C:УТ 11.5.27.75 writes the bare leaf name here.
pub(super) fn subsystem_source_reference_child_name(
    subsystem_ref: &SubsystemSourceReference,
) -> Option<String> {
    Some(
        subsystem_ref
            .relative_path
            .file_stem()?
            .to_str()?
            .to_string(),
    )
}

#[allow(dead_code)]
pub(super) fn build_metadata_field_reference_index(rows: &[ConfigRow]) -> BTreeMap<String, String> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_metadata_field_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_metadata_field_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for row in rows {
        for header in nested_metadata_headers_from_text(&row.text, &row.file_name) {
            index.insert(header.uuid, header.name);
        }
    }
    index
}

/// The single reference type each nested metadata child is *declared* to hold,
/// keyed by that child's uuid.
///
/// The child names themselves already have an index -- this is its type-side
/// twin, built from the same walk over the same headers. A child that declares
/// no type, several types, or a non-reference type is left out entirely rather
/// than approximated: the only consumer dereferences the declared type to reach
/// its standard attributes, and a value with more than one possible type has no
/// single set of those.
/// The one platform type this index has to name in its own right.
///
/// A data-composition settings composer is not a configuration type, so it is
/// absent from the type index and the child that declares it came out with no
/// declared type at all -- which is what left the settings-composer route with
/// nothing to check a chain segment against.  The row names exactly that one
/// platform type; every other builtin stays out, so no other child changes its
/// answer.
fn settings_composer_builtin_type_reference(type_id: &str) -> Option<&'static str> {
    builtin_type_reference(type_id).or_else(|| {
        type_id
            .eq_ignore_ascii_case(DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_UUID)
            .then_some(DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_NAME)
    })
}

pub(super) fn build_metadata_field_type_reference_index_from_texts(
    rows: &[MetadataTextRow],
    type_index: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for row in rows {
        for (header, marker_start) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            let value_types = parse_metadata_child_value_types_with_builtin(
                &row.text,
                marker_start,
                &header.uuid,
                type_index,
                settings_composer_builtin_type_reference,
            );
            let [ConstantValueType::Reference { reference }] = value_types.as_slice() else {
                continue;
            };
            index.insert(header.uuid, reference.clone());
        }
    }
    index
}

/// The *ordered* names of every information-register dimension that declares
/// itself master, keyed by the register's uuid.
///
/// The alphabetic field index beside it cannot serve this: it sorts its keys, it
/// mixes resources and attributes in with the dimensions, and it carries no
/// master flag. The one consumer needs the declaration order because it selects
/// a dimension positionally.
///
/// Order and flag both come from the metadata emitter's own calls -- the same
/// header walk and the same `parse_information_register_child_payload` that
/// decides what the register's `Dimension` elements say -- so the index cannot
/// name a dimension the export does not write, nor order them differently.
pub(super) fn build_information_register_master_dimension_index_from_texts(
    rows: &[MetadataTextRow],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    preserve_raw_data_paths: bool,
) -> InformationRegisterMasterDimensionIndex {
    let mut index = InformationRegisterMasterDimensionIndex::new();
    for row in rows {
        let (Some("InformationRegister"), Some(register)) =
            (row.kind.as_deref(), row.header.as_ref())
        else {
            continue;
        };
        let mut masters = Vec::new();
        for (field, marker_start) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            let Some(tag) =
                register_child_object_tag("InformationRegister", &row.text, marker_start)
            else {
                continue;
            };
            if tag != "Dimension" {
                continue;
            }
            let Some((_, properties)) = parse_information_register_child_payload(
                &row.text,
                marker_start,
                &field,
                &register.name,
                tag,
                type_index,
                object_refs,
                form_refs,
                preserve_raw_data_paths,
            ) else {
                continue;
            };
            if properties.master == Some(true) {
                masters.push(field.name);
            }
        }
        if !masters.is_empty() {
            index.insert(register.uuid.clone(), masters);
        }
    }
    index
}

pub(super) fn build_information_register_field_reference_index_from_texts(
    rows: &[MetadataTextRow],
    type_index: &BTreeMap<String, String>,
    type_set_leaves: &MetadataTypeSetLeafIndex,
) -> InformationRegisterFieldReferenceIndex {
    let mut fields_by_register = BTreeMap::<String, BTreeMap<String, BTreeSet<String>>>::new();
    for row in rows {
        let (Some("InformationRegister"), Some(register)) =
            (row.kind.as_deref(), row.header.as_ref())
        else {
            continue;
        };
        for (field, marker_start) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            let Some(tag) =
                register_child_object_tag("InformationRegister", &row.text, marker_start)
            else {
                continue;
            };
            let Some(value_types) = parse_information_register_child_value_types(
                &row.text,
                marker_start,
                &field,
                tag,
                type_index,
            ) else {
                continue;
            };
            let value_owner_references =
                information_register_value_owner_references(&value_types, type_set_leaves);
            if value_owner_references.is_empty() {
                continue;
            }
            fields_by_register
                .entry(register.uuid.clone())
                .or_default()
                .entry(format!(
                    "InformationRegister.{}.{tag}.{}",
                    register.name, field.name
                ))
                .or_default()
                .extend(value_owner_references);
        }
    }
    fields_by_register
        .into_iter()
        .map(|(register_uuid, fields)| {
            (
                register_uuid,
                fields
                    .into_iter()
                    .map(|(field_reference, value_owner_references)| {
                        InformationRegisterFieldReference {
                            field_reference,
                            value_owner_references,
                        }
                    })
                    .collect(),
            )
        })
        .collect()
}

/// The leaves every *named type set* of the configuration declares.
///
/// A `cfg:DefinedType.X` or a `cfg:Characteristic.X` is a name, not a type: the
/// leaves it stands for are declared by another metadata object, and nothing in
/// the bytes of a field that carries the name says what they are. Two readers
/// need exactly that declaration -- the information-register value-owner route,
/// which used to keep the owners of the reference leaves and throw the leaves
/// away, and the `<FillValue>` writer, which has to know whether a string is
/// among them. Keeping the leaves keeps one table: the owners are derived from
/// them where they are needed, so the two readers cannot drift apart.
pub(super) fn build_metadata_type_set_leaf_index_from_texts(
    rows: &[MetadataTextRow],
    type_index: &BTreeMap<String, String>,
) -> MetadataTypeSetLeafIndex {
    rows.iter()
        .filter_map(|row| {
            let header = row.header.as_ref()?;
            // A defined type is recognised the way its own writer recognises
            // it -- object code `0` plus the header/pattern shape -- and not
            // through `row.kind`, which never spells `DefinedType`: object code
            // `0` with the header in field 1 is a functional-option parameter,
            // a language or an integration service, and a defined type carries
            // its header in field 3 instead. The predecessor index keyed off
            // `row.kind` and was therefore empty on every configuration of the
            // stand.
            if row.object_code == Some(0)
                && is_defined_type_metadata_text(&row.text, &row.file_name)
            {
                let properties =
                    parse_defined_type_properties_from_text(&row.text, &header.uuid, type_index)?;
                return Some((
                    format!("cfg:DefinedType.{}", header.name),
                    properties.value_types,
                ));
            }
            // The characteristic type set of a chart of characteristic types is
            // the chart's own declared `<Type>`, read from the very same slot
            // and with the very same pattern parser the chart's own writer
            // reads it with.
            if row.kind.as_deref() == Some("ChartOfCharacteristicTypes") {
                let value_types = chart_of_characteristic_types_declared_value_types(
                    &row.text, header, type_index,
                )?;
                return Some((format!("cfg:Characteristic.{}", header.name), value_types));
            }
            None
        })
        .collect()
}

/// Whether the declared leaves of a type list provably exclude the `String`
/// type.
///
/// A leaf that names a type set is resolved through the index and its own
/// leaves are read in its place; a name the index cannot answer, or a cycle,
/// leaves the whole list undecided, and so does an empty list. Only a decided
/// list answers `true`.
pub(super) fn metadata_declared_leaves_exclude_string(
    value_types: &[ConstantValueType],
    type_set_leaves: &MetadataTypeSetLeafIndex,
) -> bool {
    fn walk(
        value_types: &[ConstantValueType],
        type_set_leaves: &MetadataTypeSetLeafIndex,
        visiting: &mut BTreeSet<String>,
    ) -> Option<bool> {
        if value_types.is_empty() {
            return None;
        }
        let mut excludes = true;
        for value_type in value_types {
            match value_type {
                ConstantValueType::Boolean
                | ConstantValueType::Number { .. }
                | ConstantValueType::DateTime { .. }
                | ConstantValueType::Reference { .. } => {}
                ConstantValueType::String { .. } => excludes = false,
                ConstantValueType::ReferenceTypeSet { reference } => {
                    let leaves = type_set_leaves.get(reference)?;
                    if !visiting.insert(reference.clone()) {
                        return None;
                    }
                    let nested = walk(leaves, type_set_leaves, visiting);
                    visiting.remove(reference);
                    excludes &= nested?;
                }
            }
        }
        Some(excludes)
    }

    walk(value_types, type_set_leaves, &mut BTreeSet::new()).unwrap_or(false)
}

pub(super) fn information_register_value_owner_references(
    value_types: &[ConstantValueType],
    type_set_leaves: &MetadataTypeSetLeafIndex,
) -> BTreeSet<String> {
    value_types
        .iter()
        .flat_map(|value_type| match value_type {
            ConstantValueType::Reference { reference } => {
                parse_generated_metadata_reference_owner(reference)
                    .map(|owner| owner.owner_reference())
                    .into_iter()
                    .collect::<BTreeSet<_>>()
            }
            // The owners of a named set are the owners of the leaves it
            // declares -- derived here rather than stored beside them, so the
            // leaves and their owners are one fact read one way.
            ConstantValueType::ReferenceTypeSet { reference } => type_set_leaves
                .get(reference)
                .map(|leaves| {
                    leaves
                        .iter()
                        .filter_map(|value_type| match value_type {
                            ConstantValueType::Reference { reference } => {
                                parse_generated_metadata_reference_owner(reference)
                                    .map(|owner| owner.owner_reference())
                            }
                            _ => None,
                        })
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default(),
            _ => BTreeSet::new(),
        })
        .collect()
}

#[allow(dead_code)]
pub(super) fn build_form_source_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, FormSourceReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_form_source_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_form_source_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, FormSourceReference> {
    let mut forms = Vec::<MetadataHeader>::new();
    let mut owner_paths_by_ref = BTreeMap::<String, BTreeSet<PathBuf>>::new();

    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name) {
            if let Some(header) = row.header.as_ref() {
                forms.push(header.clone());
            }
        }
    }
    let form_uuids = forms
        .iter()
        .map(|form| form.uuid.clone())
        .collect::<BTreeSet<_>>();

    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name) {
            continue;
        }
        let (Some(kind), Some(folder), Some(header)) =
            (row.kind.as_deref(), row.folder, row.header.as_ref())
        else {
            continue;
        };
        if !metadata_kind_can_own_forms(kind) {
            continue;
        }
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        let Some(references) = owned_form_uuid_values_matching(&row.text, &form_uuids) else {
            continue;
        };
        for reference in references {
            owner_paths_by_ref
                .entry(reference)
                .or_default()
                .insert(owner_path.clone());
        }
    }

    let mut index = BTreeMap::new();
    for form in forms {
        let owner_matches = owner_paths_by_ref.get(&form.uuid).map(BTreeSet::iter);
        let relative_path = if let Some(mut owner_paths) = owner_matches {
            let first = owner_paths.next();
            let second = owner_paths.next();
            if let (Some(owner_path), None) = (first, second) {
                owner_path
                    .join("Forms")
                    .join(sanitize_source_path_segment(&form.name))
                    .with_extension("xml")
            } else {
                PathBuf::from("CommonForms")
                    .join(sanitize_source_path_segment(&form.name))
                    .with_extension("xml")
            }
        } else {
            PathBuf::from("CommonForms")
                .join(sanitize_source_path_segment(&form.name))
                .with_extension("xml")
        };
        let kind = if relative_path.starts_with("CommonForms") {
            "CommonForm"
        } else {
            "Form"
        };
        index.insert(
            form.uuid,
            FormSourceReference {
                relative_path,
                kind,
            },
        );
    }

    index
}

pub(super) fn build_form_owner_resolution_diagnostics_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, String> {
    let mut forms = Vec::<MetadataHeader>::new();
    let mut owner_paths_by_ref = BTreeMap::<String, BTreeSet<PathBuf>>::new();

    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name) {
            if let Some(header) = row.header.as_ref() {
                forms.push(header.clone());
            }
        }
    }
    let form_uuids = forms
        .iter()
        .map(|form| form.uuid.clone())
        .collect::<BTreeSet<_>>();

    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name) {
            continue;
        }
        let (Some(kind), Some(folder), Some(header)) =
            (row.kind.as_deref(), row.folder, row.header.as_ref())
        else {
            continue;
        };
        if !metadata_kind_can_own_forms(kind) {
            continue;
        }
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        let Some(references) = owned_form_uuid_values_matching(&row.text, &form_uuids) else {
            continue;
        };
        for reference in references {
            owner_paths_by_ref
                .entry(reference)
                .or_default()
                .insert(owner_path.clone());
        }
    }

    let mut diagnostics = BTreeMap::new();
    for form in forms {
        let owner_paths = owner_paths_by_ref.get(&form.uuid);
        let owner_count = owner_paths.map_or(0, BTreeSet::len);
        if owner_count == 1 {
            continue;
        }

        let candidates = owner_paths
            .map(|paths| {
                paths
                    .iter()
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let candidates = if candidates.is_empty() {
            "none".to_string()
        } else {
            candidates.join(", ")
        };
        diagnostics.insert(
            format!("{}.0", form.uuid),
            format!(
                "form \"{}\" ({}) owner resolution expected exactly 1 owner, found {}; candidates: {}; fallback path: CommonForms/{}.xml",
                form.name,
                form.uuid,
                owner_count,
                candidates,
                sanitize_source_path_segment(&form.name)
            ),
        );
    }

    diagnostics
}

// Platform-level 1C markers for owned form lists in metadata blobs. These are
// not configuration object UUIDs and must not be replaced with DB-specific IDs.
#[allow(dead_code)]
pub(super) fn build_template_source_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, TemplateSourceReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_template_source_reference_index_from_texts(rows, &metadata_texts)
}

pub(super) fn build_template_source_reference_index_from_texts(
    rows: &[ConfigRow],
    metadata_texts: &[MetadataTextRow],
) -> BTreeMap<String, TemplateSourceReference> {
    let rows_by_file_name = rows
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut templates = Vec::<MetadataHeader>::new();
    let mut owner_paths_by_ref = BTreeMap::<String, Vec<PathBuf>>::new();

    for row in metadata_texts {
        if is_template_metadata_text(&row.text, &row.file_name) {
            if let Some(header) = row.header.as_ref() {
                templates.push(header.clone());
            }
            continue;
        }
        let (Some(kind), Some(folder), Some(header)) =
            (row.kind.as_deref(), row.folder, row.header.as_ref())
        else {
            continue;
        };
        if !metadata_kind_can_own_templates(kind) {
            continue;
        }
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        for reference in uuid_like_values(&row.text) {
            owner_paths_by_ref
                .entry(reference)
                .or_default()
                .push(owner_path.clone());
        }
    }

    let mut index = BTreeMap::new();
    for template in templates {
        let owner_matches = owner_paths_by_ref
            .get(&template.uuid)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let relative_path = if let [owner_path] = owner_matches {
            owner_path
                .join("Templates")
                .join(sanitize_source_path_segment(&template.name))
                .with_extension("xml")
        } else {
            PathBuf::from("CommonTemplates")
                .join(sanitize_source_path_segment(&template.name))
                .with_extension("xml")
        };
        let kind = if relative_path.starts_with("CommonTemplates") {
            "CommonTemplate"
        } else {
            "Template"
        };
        let body_id = format!("{}.0", template.uuid);
        let template_type = template_template_type_from_metadata(&template)
            .or_else(|| {
                rows_by_file_name
                    .get(body_id.as_str())
                    .and_then(|row| decode_hex(&row.binary_hex).ok())
                    .and_then(|bytes| infer_template_type_from_body(&bytes))
            })
            .unwrap_or("BinaryData");
        index.insert(
            template.uuid,
            TemplateSourceReference {
                relative_path,
                kind,
                template_type,
            },
        );
    }

    index
}

#[allow(dead_code)]
pub(super) fn build_subsystem_source_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, SubsystemSourceReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_subsystem_source_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_subsystem_source_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, SubsystemSourceReference> {
    let mut subsystems = BTreeMap::<String, (MetadataHeader, String)>::new();

    for row in rows {
        let Some(kind) = row.kind.as_deref() else {
            continue;
        };
        if kind != "Subsystem" {
            continue;
        }
        let Some(header) = row.header.as_ref() else {
            continue;
        };
        subsystems.insert(header.uuid.clone(), (header.clone(), row.text.clone()));
    }

    let subsystem_uuids = subsystems.keys().cloned().collect::<BTreeSet<_>>();
    let mut owners_by_child = BTreeMap::<String, Vec<String>>::new();
    for (owner_uuid, (_, owner_text)) in &subsystems {
        for reference in uuid_like_values(owner_text) {
            if reference != *owner_uuid && subsystem_uuids.contains(&reference) {
                owners_by_child
                    .entry(reference)
                    .or_default()
                    .push(owner_uuid.clone());
            }
        }
    }
    let mut parent_by_child = BTreeMap::<String, String>::new();
    for (child_uuid, owners) in owners_by_child {
        if let [owner_uuid] = owners.as_slice() {
            parent_by_child.insert(child_uuid, owner_uuid.clone());
        }
    }

    let mut memo = BTreeMap::<String, PathBuf>::new();
    for uuid in subsystems.keys() {
        let mut visiting = BTreeSet::<String>::new();
        let _ = resolve_subsystem_source_path(
            uuid,
            &subsystems,
            &parent_by_child,
            &mut memo,
            &mut visiting,
        );
    }

    memo.into_iter()
        .map(|(uuid, relative_path)| (uuid, SubsystemSourceReference { relative_path }))
        .collect()
}

pub(super) fn resolve_subsystem_source_path(
    uuid: &str,
    subsystems: &BTreeMap<String, (MetadataHeader, String)>,
    parent_by_child: &BTreeMap<String, String>,
    memo: &mut BTreeMap<String, PathBuf>,
    visiting: &mut BTreeSet<String>,
) -> Option<PathBuf> {
    if let Some(path) = memo.get(uuid) {
        return Some(path.clone());
    }
    if !visiting.insert(uuid.to_string()) {
        return None;
    }
    let (header, _) = subsystems.get(uuid)?;
    let name = sanitize_source_path_segment(&header.name);
    let relative_path = if let Some(parent_uuid) = parent_by_child.get(uuid) {
        resolve_subsystem_source_path(parent_uuid, subsystems, parent_by_child, memo, visiting)
            .map(|parent_path| {
                parent_path
                    .with_extension("")
                    .join("Subsystems")
                    .join(&name)
                    .with_extension("xml")
            })
            .unwrap_or_else(|| {
                PathBuf::from("Subsystems")
                    .join(&name)
                    .with_extension("xml")
            })
    } else {
        PathBuf::from("Subsystems")
            .join(&name)
            .with_extension("xml")
    };
    visiting.remove(uuid);
    memo.insert(uuid.to_string(), relative_path.clone());
    Some(relative_path)
}

pub(super) fn uuid_like_values(text: &str) -> BTreeSet<String> {
    uuid_like_values_in_text_order(text).into_iter().collect()
}

pub(super) fn uuid_like_values_in_text_order(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut values = Vec::new();
    let mut seen = BTreeSet::new();
    if bytes.len() < 36 {
        return values;
    }
    for start in 0..=bytes.len() - 36 {
        let value = &bytes[start..start + 36];
        if is_uuid_like_ascii(value) {
            let value = String::from_utf8_lossy(value).to_ascii_lowercase();
            if seen.insert(value.clone()) {
                values.push(value);
            }
        }
    }
    values
}

pub(super) fn is_uuid_like_ascii(value: &[u8]) -> bool {
    if value.len() != 36 {
        return false;
    }
    for (index, byte) in value.iter().copied().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

pub(super) fn infer_template_type_from_body(bytes: &[u8]) -> Option<&'static str> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    if inflated.starts_with(b"MOXCEL") {
        return Some("SpreadsheetDocument");
    }
    let Ok(text) = std::str::from_utf8(&inflated) else {
        return Some("BinaryData");
    };
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let xml_text = text
        .starts_with("<?xml")
        .then_some(text)
        .or_else(|| text.find("<?xml").map(|index| &text[index..]));
    if xml_text.is_some_and(|xml| xml.contains("data-composition-system/appearance-template")) {
        Some("DataCompositionAppearanceTemplate")
    } else if xml_text.is_some_and(|xml| xml.contains("data-composition-system/schema")) {
        Some("DataCompositionSchema")
    } else if xml_text.is_some_and(|xml| xml.contains("8.3/xcf/scheme")) {
        Some("GraphicalSchema")
    } else if text.starts_with("<!DOCTYPE")
        || text.starts_with("<html")
        || text.starts_with("<?xml") && text.contains("<html")
    {
        Some("HTMLDocument")
    } else if looks_like_graphical_scheme_blob_text(text) {
        // Real ERP UH bytes: a `GraphicalSchema` Template body is not
        // always pre-serialized XML (the three markers above cover that
        // case) -- the platform also stores it in the same brace-tuple
        // grammar `BusinessProcess.Flowchart`'s `Ext/Flowchart.xml` decodes
        // from (`parse_business_process_flowchart_text_with_types`; see
        // `flowchart_grammar_fields`'s doc comment). Falling through to
        // `TextDocument` here (the previous behavior) wrote every one of
        // these at `Templates/<name>/Ext/Template.txt` -- a path the
        // platform never produces -- while the platform's own
        // `Templates/<name>/Ext/Template.xml` sat unwritten: the entire
        // evidenced `extra` class documented in `output-path-collisions-
        // and-module-text-fallback-20260825.md` section 4. Recognizing the
        // shape here only fixes the *type* (and so the output path); the
        // write-time decode can still fail on an item shape this project's
        // flowchart parser does not yet model (e.g. a `Decoration` with a
        // `Picture`), which is a typed failure, not a second wrong default.
        Some("GraphicalSchema")
    } else {
        Some("TextDocument")
    }
}

pub(super) fn template_template_type_from_metadata(
    header: &MetadataHeader,
) -> Option<&'static str> {
    template_type_from_code(header.template_type_code?)
}

pub(super) fn template_type_from_code(code: u32) -> Option<&'static str> {
    match code {
        0 => Some("SpreadsheetDocument"),
        1 => Some("BinaryData"),
        3 => Some("HTMLDocument"),
        4 => Some("TextDocument"),
        6 => Some("DataCompositionSchema"),
        7 => Some("DataCompositionAppearanceTemplate"),
        // Confirmed on real ERP УХ 3.2.12.6 raw metadata via `cf extract` on
        // two owned templates whose native XML declares
        // `<TemplateType>GraphicalSchema</TemplateType>`:
        // `DataProcessors/ВыполнениеМаршрутныхЛистов/Templates/МетодикаББВ`
        // (uuid b882ff8c-b85a-4c2a-bcd9-4321c2dbb154) and
        // `Reports/АнализСостоянияНалоговогоУчетаПоНДС/Templates/УчетНДС`
        // (uuid 325ac0f9-1bc2-49d9-8a62-c9fe1a988830) both carry raw code 8
        // (`{2,8,{3,{1,0,<uuid>},...}}`). Neither БСП demo/base nor УТ
        // 11.5.27.75 ever writes this code among their owned templates, so it
        // was previously unmapped and every such template fell back through
        // body content-sniffing to `BinaryData` -- wrong, since a
        // GraphicalSchema body is not always distinguishable as such by
        // sniffing raw bytes.
        8 => Some("GraphicalSchema"),
        9 => Some("AddIn"),
        _ => None,
    }
}

pub(super) fn form_help_asset_paths(
    rows: &[ConfigRow],
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> BTreeMap<String, SourceAsset> {
    let file_names = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut paths = BTreeMap::new();
    for (form_uuid, form_ref) in form_refs {
        let row_prefix = format!("{form_uuid}.");
        let mut form_dir = form_ref.relative_path.clone();
        form_dir.set_extension("");
        for body_id in file_names
            .iter()
            .filter(|file_name| file_name.starts_with(&row_prefix))
        {
            let module_body_id = format!("{form_uuid}.0");
            if *body_id == module_body_id.as_str() {
                continue;
            }
            if let Some(row) = rows_by_file_name.get(*body_id)
                && let Ok(bytes) = decode_hex(&row.binary_hex)
                && parse_help_blob_pages(&bytes).is_some()
            {
                paths.insert(
                    (*body_id).to_string(),
                    SourceAsset {
                        primary_path: form_dir.join("Ext").join("Help.xml"),
                        kind: SourceAssetKind::Help,
                    },
                );
            }
        }
    }
    paths
}

#[allow(dead_code)]
pub(super) fn parse_configuration_reference_blob(blob: &[u8]) -> Option<String> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    parse_configuration_reference_text(text)
}

pub(super) fn parse_configuration_reference_text(text: &str) -> Option<String> {
    parse_configuration_reference_text_with_identity(text).map(|(_, name)| name)
}

pub(super) fn parse_configuration_reference_text_for_row(
    text: &str,
    expected_identity: &str,
) -> Option<String> {
    let (identity, name) = parse_configuration_reference_text_with_identity(text)?;
    identity
        .eq_ignore_ascii_case(expected_identity)
        .then_some(name)
}

fn parse_configuration_reference_text_with_identity(text: &str) -> Option<(String, String)> {
    let envelope = parse_configuration_root_envelope(text)?;
    let header_uuid = parse_configuration_header_uuid(text)?;
    let header = parse_metadata_header_from_text(text, &header_uuid)?;
    Some((envelope.identity, header.name))
}

pub(super) fn extract_configuration_source_xml(
    text: &str,
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
    source_version: InfobaseConfigSourceVersion,
) -> Option<String> {
    if !text.trim_start().starts_with("{2,") {
        return None;
    }
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    let uuid_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    if uuid_fields.first()?.trim() != uuid {
        return None;
    }
    let header_uuid = parse_configuration_header_uuid(text)?;
    let mut header = parse_metadata_header_from_text(text, &header_uuid)?;
    header.uuid = uuid.to_string();
    let mut properties =
        parse_configuration_properties_from_text(text, object_refs).unwrap_or_default();
    properties.use_purposes = parse_configuration_use_purposes(text, uuid).unwrap_or_default();
    let evidenced_property_fields = configuration_root_property_fields(text, uuid);
    if let Some(property_fields) = evidenced_property_fields.as_deref() {
        match super::configuration_properties_evidence::parse_configuration_properties_evidenced_default_block(
            property_fields,
        ) {
            Ok(fields) => properties.configuration_properties_evidenced_default_block = Some(fields),
            Err(
                super::configuration_properties_evidence::ConfigurationPropertiesEvidenceError::UnexpectedTupleArity { .. }
                | super::configuration_properties_evidence::ConfigurationPropertiesEvidenceError::UnexpectedFieldShape { .. }
                | super::configuration_properties_evidence::ConfigurationPropertiesEvidenceError::UnexpectedFieldClass { .. },
            ) => {
                // This tuple doesn't have the shape the proven field indices
                // were established against at all (e.g. a synthetic/SQL-sourced
                // text a non-CF caller constructed) -- never proven to be the
                // evidenced family, so fall back to today's existing per-field
                // behavior rather than fail closed on a coordinate we have no
                // evidence about either way.
            }
            Err(
                super::configuration_properties_evidence::ConfigurationPropertiesEvidenceError::UnrecognizedDigit { .. }
                | super::configuration_properties_evidence::ConfigurationPropertiesEvidenceError::UnprovenFieldMismatch { .. },
            ) => {
                // The tuple DOES have the evidenced family's shape, but a
                // field we cannot yet decode diverges from the proven
                // all-default reference -- fail closed (no XML for this
                // Configuration object at all) rather than silently repeat
                // the incomplete-but-quiet omission of ~30 Properties fields.
                return None;
            }
        }
    }
    if configuration_root_property_fields(text, uuid).is_some() {
        properties.default_roles =
            parse_configuration_default_roles_from_root(text, uuid, object_refs)
                .unwrap_or_default();
        properties.brief_information.clear();
        properties.detailed_information.clear();
        properties.copyright.clear();
        properties.vendor_information_address.clear();
        properties.configuration_information_address.clear();
        properties.localized_properties =
            parse_configuration_localized_properties_from_root(text, uuid);
    }
    let (functionalities, permission_messages) =
        parse_configuration_used_mobile_application_functionalities(
            text,
            uuid,
            source_version.as_str(),
        )
        .unwrap_or_default();
    properties.used_mobile_application_functionalities = functionalities;
    properties.used_mobile_application_permission_messages = permission_messages;
    if let Some(property_fields) = evidenced_property_fields.as_deref() {
        let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();
        for (slot, target) in [
            (
                policy.default_report_form_tuple_field(),
                &mut properties.default_report_form,
            ),
            (
                policy.default_report_variant_form_tuple_field(),
                &mut properties.default_report_variant_form,
            ),
            (
                policy.default_report_settings_form_tuple_field(),
                &mut properties.default_report_settings_form,
            ),
        ] {
            *target = parse_configuration_root_reference_slot(
                property_fields,
                slot,
                object_refs,
                "CommonForm.",
            );
        }
    }
    let root_layout = parse_configuration_root_layout(text, uuid);
    let child_objects = root_layout
        .is_none()
        .then(|| parse_configuration_child_objects(text, uuid, &header_uuid))
        .unwrap_or_default();
    let mut xml = format_configuration_source_xml(&header, &properties, source_version);
    if let Some(root_layout) = &root_layout {
        insert_configuration_internal_info_xml(&mut xml, &root_layout.contained_objects).ok()?;
        if let Some(child_objects) =
            resolve_configuration_root_child_objects(root_layout, object_refs)
        {
            insert_configuration_root_child_objects_xml(&mut xml, &child_objects);
        }
    } else if !child_objects.is_empty() {
        let mut child_xml = String::new();
        for child_object in &child_objects {
            push_metadata_header_child_object_xml(
                &mut child_xml,
                child_object.tag,
                &child_object.header,
            );
        }
        insert_metadata_child_objects_xml(&mut xml, "Configuration", &child_xml);
    }
    Some(xml)
}

pub(super) fn parse_configuration_properties_from_text(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<ConfigurationProperties> {
    let (fields, is_native_68_shape) = configuration_root_fields(text)?;
    // Field 26 mirroring field 43 (`CompatibilityMode`, below) into
    // `ConfigurationExtensionCompatibilityMode` is proven only for the
    // genuine `{68,...}` shape -- three corpora, "none match their own field
    // 43 ... under `configuration_compatibility_mode_xml`'s formula", i.e.
    // field 26 was the only working coordinate for either property there.
    // SSL/БСП 3.1.12.297's `{67,...}` short revision (which
    // `configuration_root_fields` normalizes into this same 61-field shape,
    // leaving fields 1..59 -- 26 and 43 among them -- at their original
    // tuple positions) is a different record altogether: both `sslbase`'s
    // and `ssl`-demo's copies carry `80324` at *both* fields 26 and 43, and
    // native prints `CompatibilityMode` `Version8_3_24` (matching field 43
    // directly, not mirrored) alongside `ConfigurationExtensionCompatibilityMode`
    // `Version8_3_27` -- a value present nowhere in the tuple. `Version8_3_27`
    // is exactly this reading platform's own build
    // (`MAX_EVIDENCED_PACKED_PLATFORM_VERSION`, `ibcmd` 8.3.27.2214): the
    // shorter `{67,...}` shape predates the extension-compatibility property,
    // so a config still in that shape has no explicit value for it and the
    // platform substitutes its own version when reading it back, while field
    // 43 there -- unlike on the genuine `{68,...}` shape -- is this record's
    // own faithful `CompatibilityMode` storage, not a stale alias of field 26.
    //
    // `configuration_root_fields` is a blind text search, not the
    // uuid-anchored kind `configuration_root_property_fields` does: a
    // coincidental `{67,` match elsewhere in the same text (e.g. an
    // unrelated metadata header shaped `{67,{0,{3,...}}}`, 2 top-level
    // members) also comes back with `is_native_68_shape == false` without
    // ever having been the genuine, 60-field short Properties tuple.
    // `normalize_short_configuration_root_property_fields` only pads to the
    // full 61-field shape when the match really was `("67", 60)`; anything
    // else it returns unchanged and short. Requiring the post-normalization
    // length here tells the two apart instead of defaulting off of a match
    // that was never this record to begin with.
    let is_normalized_67_shape = !is_native_68_shape && fields.len() == 61;
    let configuration_extension_compatibility_mode = if is_native_68_shape {
        fields
            .get(26)
            .and_then(|field| configuration_compatibility_mode_xml(field.trim()))
    } else if is_normalized_67_shape {
        configuration_compatibility_mode_xml(&MAX_EVIDENCED_PACKED_PLATFORM_VERSION.to_string())
    } else {
        None
    };
    let compatibility_mode = if is_native_68_shape {
        configuration_extension_compatibility_mode.clone()
    } else if is_normalized_67_shape {
        fields
            .get(43)
            .and_then(|field| configuration_compatibility_mode_xml(field.trim()))
    } else {
        None
    };
    Some(ConfigurationProperties {
        name_prefix: fields
            .get(2)
            .and_then(|field| parse_1c_quoted_string(field.trim())),
        configuration_extension_compatibility_mode,
        default_run_mode: fields
            .get(3)
            .and_then(|field| configuration_default_run_mode_xml(field.trim())),
        use_purposes: Vec::new(),
        localized_properties: None,
        brief_information: parse_configuration_localized_property(&fields, 4),
        detailed_information: parse_configuration_localized_property(&fields, 5),
        copyright: parse_configuration_localized_property(&fields, 6),
        vendor_information_address: parse_configuration_localized_property(&fields, 7),
        configuration_information_address: parse_configuration_localized_property(&fields, 8),
        default_style: parse_configuration_root_reference(&fields, 9, object_refs, "Style."),
        default_language: parse_configuration_root_reference(&fields, 10, object_refs, "Language."),
        script_variant: fields
            .get(13)
            .and_then(|field| configuration_script_variant_xml(field.trim())),
        default_roles: fields
            .get(39)
            .map(|field| parse_configuration_default_roles(field, object_refs))
            .unwrap_or_default(),
        vendor: fields
            .get(14)
            .and_then(|field| parse_1c_quoted_string(field.trim())),
        version: fields
            .get(15)
            .and_then(|field| parse_1c_quoted_string(field.trim())),
        update_catalog_address: fields
            .get(16)
            .and_then(|field| parse_1c_quoted_string(field.trim())),
        common_settings_storage: parse_configuration_root_reference_slot(
            &fields,
            22,
            object_refs,
            "SettingsStorage.",
        ),
        reports_user_settings_storage: parse_configuration_root_reference_slot(
            &fields,
            23,
            object_refs,
            "SettingsStorage.",
        ),
        reports_variants_storage: parse_configuration_root_reference_slot(
            &fields,
            24,
            object_refs,
            "SettingsStorage.",
        ),
        form_data_settings_storage: parse_configuration_root_reference_slot(
            &fields,
            25,
            object_refs,
            "SettingsStorage.",
        ),
        default_report_form: None,
        default_report_variant_form: None,
        default_report_settings_form: None,
        used_mobile_application_functionalities: Vec::new(),
        used_mobile_application_permission_messages: Vec::new(),
        // See the evidence above `configuration_extension_compatibility_mode`:
        // on the genuine `{68,` shape `CompatibilityMode` mirrors that same
        // field 26 read (proven on three corpora -- ERP УХ's
        // `Web_Service.cf` and `MDM_Management.cf`, both field 43 = `80307`
        // against native `Version8_3_27`, and sitec's
        // `МодульWebОбмена_ERP25.cf`, field 43 = `80501` against the same
        // native `Version8_3_27` -- none matching their own field 43 under
        // `configuration_compatibility_mode_xml`'s formula); on the shorter
        // `{67,` shape (SSL/БСП 3.1.12.297) field 43 is independently read
        // instead, since there it does hold this record's own faithful
        // value (`80324` matching native `Version8_3_24` directly).
        compatibility_mode,
        configuration_properties_evidenced_default_block: None,
    })
}

pub(super) fn parse_configuration_default_roles(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(fields) = split_1c_braced_fields(field, 0) else {
        return Vec::new();
    };
    let Some(count) = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };

    fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| parse_design_time_reference(field, object_refs))
        .filter(|reference| reference.starts_with("Role."))
        .collect()
}

pub(super) fn parse_configuration_default_roles_from_root(
    text: &str,
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<String>> {
    let fields = configuration_root_property_fields(text, uuid)?;
    let raw_fields = split_1c_braced_fields(fields.get(39)?.trim(), 0)?;
    if raw_fields.first()?.trim() != "0" {
        return None;
    }
    let count = raw_fields.get(1)?.trim().parse::<usize>().ok()?;
    if raw_fields.len() != count.checked_add(2)? {
        return None;
    }

    let mut seen_uuids = BTreeSet::new();
    let mut seen_references = BTreeSet::new();
    let mut roles = Vec::with_capacity(count);
    for raw_role in raw_fields.iter().skip(2) {
        let role_fields = split_1c_braced_fields(raw_role.trim(), 0)?;
        if role_fields.len() != 3
            || parse_1c_quoted_string(role_fields.first()?.trim()).as_deref() != Some("#")
            || role_fields.get(1)?.trim() != METADATA_OBJECT_REF_TYPE_UUID
        {
            return None;
        }
        let target_fields = split_1c_braced_fields(role_fields.get(2)?.trim(), 0)?;
        if target_fields.len() != 2 || target_fields.first()?.trim() != "1" {
            return None;
        }
        let role_uuid = parse_non_zero_uuid(target_fields.get(1)?.trim())?;
        if !seen_uuids.insert(role_uuid.clone()) {
            return None;
        }
        let reference = object_refs.get(&role_uuid)?;
        let (kind, name) = reference.split_once('.')?;
        if kind != "Role" || name.is_empty() || name.contains('.') {
            return None;
        }
        if !seen_references.insert(reference.clone()) {
            return None;
        }
        roles.push(reference.clone());
    }
    Some(roles)
}

pub(super) fn parse_configuration_localized_properties_from_root(
    text: &str,
    uuid: &str,
) -> Option<ConfigurationLocalizedProperties> {
    let fields = configuration_root_property_fields(text, uuid)?;
    Some(ConfigurationLocalizedProperties {
        // WMS5's `МодульWebОбмена_ERP25.cf` proves the pairing: field 4
        // carries the longer, multi-line "for these configurations: - ..."
        // text, and native `Configuration.xml` renders that one under
        // `<DetailedInformation>`, not `<BriefInformation>` -- the two were
        // swapped here (field 5 is the short one-line text, matching
        // `<BriefInformation>`'s native content byte for byte).
        detailed_information: parse_configuration_localized_property_field(fields.get(4)?)?,
        brief_information: parse_configuration_localized_property_field(fields.get(5)?)?,
        copyright: parse_configuration_localized_property_field(fields.get(6)?)?,
        vendor_information_address: parse_configuration_localized_property_field(fields.get(7)?)?,
        configuration_information_address: parse_configuration_localized_property_field(
            fields.get(8)?,
        )?,
    })
}

fn parse_configuration_localized_property_field(field: &str) -> Option<Vec<(String, String)>> {
    let raw_fields = split_1c_braced_fields(field.trim(), 0)?;
    let count = raw_fields.first()?.trim().parse::<usize>().ok()?;
    let expected_len = count.checked_mul(2)?.checked_add(1)?;
    if raw_fields.len() != expected_len {
        return None;
    }

    let mut seen_languages = BTreeSet::new();
    let mut values = Vec::with_capacity(count);
    for pair in raw_fields[1..].chunks_exact(2) {
        let language = parse_exact_1c_quoted_string(pair.first()?.trim())?;
        if !seen_languages.insert(language.clone()) {
            return None;
        }
        let content = parse_exact_1c_quoted_string(pair.get(1)?.trim())?;
        values.push((language, content));
    }
    Some(values)
}

fn parse_exact_1c_quoted_string(field: &str) -> Option<String> {
    let field = field.trim();
    let (value, consumed) = parse_1c_quoted_string_with_len(field)?;
    (consumed == field.len()).then_some(value)
}

/// The Configuration root's own `<Properties>` tuple, normalized to the
/// canonical `{68,...}` (61-field) shape `parse_configuration_properties_from_text`
/// addresses by fixed index, plus whether the record was genuinely that
/// shape to begin with (`true`) rather than a short `{67,...}` one this
/// function normalized (`false`) -- some of that shape's field coordinates
/// (see `parse_configuration_properties_from_text`) are proven only for the
/// real `{68,...}` record.
///
/// `configuration_root_property_fields` (the uuid-anchored sibling reader)
/// already proves the second shape: `{67,...}` at 60 fields, evidenced by
/// SSL/БСП 3.1.12.297 (`ssl`, `sslbase`, both 3.1.12.297). Structurally, every
/// one of its 60 members lines up field-class-for-field-class (group for
/// group, quoted for quoted, uuid for uuid, scalar for scalar) against the
/// evidenced `{68,...}` reference's first 60 members, with no reordering --
/// only the reference's own trailing 61st member is missing, a tail
/// truncation like every other short item-record revision this codebase
/// normalizes. That 61st member's value is not a guess: the sibling reader's
/// own `("68", 61) if fields.get(60)?.trim() == "1"` acceptance arm already
/// proves every corpus using the `{68,...}` shape carries literal `"1"`
/// there, so restoring it for the short `{67,...}` shape reproduces the one
/// value every other evidenced corpus already has. Before this normalization,
/// the blind `{68,` search below found nothing at all in an SSL/БСП corpus,
/// dropping name_prefix/default_run_mode/script_variant/vendor/version/
/// update_catalog_address/the four settings storages/default_style/
/// default_language whole -- most of `Configuration.xml`'s remaining diff on
/// both `ssl` and `sslbase`.
pub(super) fn configuration_root_fields(text: &str) -> Option<(Vec<&str>, bool)> {
    if let Some(start) = text.find("{68,") {
        return Some((split_1c_braced_fields(text, start)?, true));
    }
    let start = text.find("{67,")?;
    let fields = split_1c_braced_fields(text, start)?;
    Some((
        normalize_short_configuration_root_property_fields(fields)?,
        false,
    ))
}

/// Rewrite a short `{67,...}` (60-field) Configuration `<Properties>` tuple
/// into its canonical `{68,...}` (61-field) shape, or return it unchanged if
/// it is already that shape (or some other one neither reader here has
/// evidence for). See `configuration_root_fields` for the evidence this
/// normalization rests on. The leading member is rewritten to `68` too: the
/// evidenced-default-block comparator (`configuration_properties_evidence.rs`)
/// requires every one of its `unproven_tuple_fields()` -- index 0 among them
/// -- to be byte-identical with the reference's, and the reference's own
/// index 0 is `"68"`.
fn normalize_short_configuration_root_property_fields(fields: Vec<&str>) -> Option<Vec<&str>> {
    if fields.first()?.trim() != "67" || fields.len() != 60 {
        return Some(fields);
    }
    let mut normalized = Vec::with_capacity(61);
    normalized.push("68");
    normalized.extend(fields[1..].iter().copied());
    normalized.push("1");
    Some(normalized)
}

const CONFIGURATION_USE_PURPOSE_TYPE_UUID: &str = "1708fdaa-cbce-4289-b373-07a5a74bee91";

pub(super) fn parse_configuration_use_purposes(
    text: &str,
    uuid: &str,
) -> Option<Vec<&'static str>> {
    let fields = configuration_root_property_fields(text, uuid)?;
    let raw_fields = split_1c_braced_fields(fields.get(33)?.trim(), 0)?;
    if raw_fields.len() != 2 || raw_fields.first()?.trim() != "1" {
        return None;
    }
    let purpose_fields = split_1c_braced_fields(raw_fields.get(1)?.trim(), 0)?;
    if purpose_fields.len() != 3
        || parse_1c_quoted_string(purpose_fields.first()?.trim()).as_deref() != Some("#")
        || purpose_fields.get(1)?.trim() != CONFIGURATION_USE_PURPOSE_TYPE_UUID
        || purpose_fields.get(2)?.trim() != "1"
    {
        return None;
    }
    Some(vec!["PlatformApplication"])
}

const CONFIGURATION_MOBILE_APPLICATION_FUNCTIONALITIES: [(u32, &str); 38] = [
    (0, "Biometrics"),
    (1, "Location"),
    (2, "BackgroundLocation"),
    (3, "BluetoothPrinters"),
    (4, "WiFiPrinters"),
    (5, "Contacts"),
    (6, "Calendars"),
    (7, "PushNotifications"),
    (8, "LocalNotifications"),
    (9, "InAppPurchases"),
    (10, "PersonalComputerFileExchange"),
    (11, "Ads"),
    (12, "NumberDialing"),
    (13, "CallProcessing"),
    (14, "CallLog"),
    (15, "AutoSendSMS"),
    (16, "ReceiveSMS"),
    (17, "SMSLog"),
    (18, "Camera"),
    (19, "Microphone"),
    (20, "MusicLibrary"),
    (21, "PictureAndVideoLibraries"),
    (22, "AudioPlaybackAndVibration"),
    (23, "BackgroundAudioPlaybackAndVibration"),
    (24, "InstallPackages"),
    (25, "OSBackup"),
    (26, "ApplicationUsageStatistics"),
    (27, "BarcodeScanning"),
    (32, "BackgroundAudioRecording"),
    (33, "AllFilesAccess"),
    (34, "Videoconferences"),
    (35, "NFC"),
    (36, "DocumentScanning"),
    (37, "SpeechToText"),
    (38, "Geofences"),
    (39, "IncomingShareRequests"),
    (40, "AllIncomingShareRequestsTypesProcessing"),
    (41, "TextToSpeech"),
];

/// The OS permissions a `<app:permissionMessage>` can explain.
///
/// A separate vocabulary from the functionality table above -- the same name
/// carries a different id in each (`Camera` is functionality 18 and
/// permission 12) -- so nothing here is inferred from that one. These seven
/// are read straight off Документооборот КОРП 3.0.21.3, the only
/// configuration on the stand whose block carries messages at all: its
/// record's seven ids line up one-to-one, in order, with the seven
/// `<app:permission>` names its native `Configuration.xml` prints. Any other
/// id fails closed.
const CONFIGURATION_MOBILE_APPLICATION_PERMISSIONS: [(u32, &str); 7] = [
    (0, "Biometrics"),
    (12, "Camera"),
    (13, "Microphone"),
    (14, "MusicLibrary"),
    (15, "PictureAndVideoLibraries"),
    (16, "AudioPlaybackAndVibration"),
    (20, "PostNotifications"),
];

fn configuration_mobile_application_permission_name(id: u32) -> Option<&'static str> {
    CONFIGURATION_MOBILE_APPLICATION_PERMISSIONS
        .iter()
        .find_map(|(candidate, name)| (*candidate == id).then_some(*name))
}

pub(super) fn parse_configuration_used_mobile_application_functionalities(
    text: &str,
    uuid: &str,
    source_version: &str,
) -> Option<(
    Vec<ConfigurationMobileApplicationFunctionality>,
    Vec<ConfigurationMobileApplicationPermissionMessage>,
)> {
    let fields = configuration_root_property_fields(text, uuid)?;
    let raw_fields = split_1c_braced_fields(fields.get(53)?.trim(), 0)?;
    if raw_fields.first()?.trim() != "2" {
        return None;
    }
    let count = raw_fields.get(1)?.trim().parse::<usize>().ok()?;
    // The pair table is followed by one scalar, and that scalar can in turn
    // be followed by a counted list of permission messages -- Документооборот
    // КОРП 3.0.21.3 writes seven, every other configuration on the stand
    // writes the scalar `0` and stops. Demanding the table end right after
    // the scalar dropped the whole block for the one that does not.
    let tail = raw_fields.get(count.checked_add(2)?..)?;
    let trailing_field = *tail.first()?;
    let permission_messages = if tail.len() == 1 {
        Vec::new()
    } else {
        parse_configuration_mobile_application_permission_messages(tail)?
    };
    let mut functionalities = Vec::with_capacity(38);
    for ((expected_id, name), field) in CONFIGURATION_MOBILE_APPLICATION_FUNCTIONALITIES
        .iter()
        .take(count)
        .zip(raw_fields.iter().skip(2).take(count))
    {
        let pair = split_1c_braced_fields(field.trim(), 0)?;
        if pair.len() != 2 || pair.first()?.trim().parse::<u32>().ok()? != *expected_id {
            return None;
        }
        functionalities.push(ConfigurationMobileApplicationFunctionality {
            name,
            use_functionality: parse_1c_bool_flag(pair.get(1)?.trim())?,
        });
    }

    // The record's own declared count decides how the table's last entry is
    // spelled; the dialect only decides whether that entry is printed. A
    // record that spells all `full` entries as pairs is complete as it
    // stands, and the shorter one carries its last entry in the trailing
    // scalar instead.
    //
    // The `2.20`+`full` arm is the one this table was missing: every
    // 8.3.27.2214 configuration retained here -- all nine bundled evidence
    // corpora and «1С:Управление торговлей 11.5.27.75» -- declares 38, spells
    // the 38 ids of the table above in order, and its flags decode to that
    // configuration's own `<UsedMobileApplicationFunctionalities>` block
    // value for value; every one of them is exported at `2.20`, where the old
    // table demanded 37 and so dropped the block outright.
    let full = CONFIGURATION_MOBILE_APPLICATION_FUNCTIONALITIES.len();
    match (source_version, count) {
        // The 2.17 dialect prints no `TextToSpeech` at all, so the shorter
        // record's trailing scalar is read for its shape and dropped. No
        // corpus shows how a full-length record maps onto that dialect.
        ("2.17", n) if n + 1 == full => {
            parse_1c_bool_flag(trailing_field.trim())?;
        }
        ("2.20", n) if n + 1 == full => {
            functionalities.push(ConfigurationMobileApplicationFunctionality {
                name: CONFIGURATION_MOBILE_APPLICATION_FUNCTIONALITIES[full - 1].1,
                use_functionality: parse_1c_bool_flag(trailing_field.trim())?,
            })
        }
        ("2.20" | "2.21", n) if n == full => {}
        _ => return None,
    }
    // The shorter record spends its trailing scalar on the last
    // functionality's flag, so it has nowhere left to declare messages. Only
    // the full-length shape is observed carrying them.
    if !permission_messages.is_empty() && count != full {
        return None;
    }
    Some((functionalities, permission_messages))
}

/// `<count>, {<permission id>, <localized description>} …`, the tail of a
/// full-length mobile-functionalities table.
fn parse_configuration_mobile_application_permission_messages(
    tail: &[&str],
) -> Option<Vec<ConfigurationMobileApplicationPermissionMessage>> {
    let count = tail.first()?.trim().parse::<usize>().ok()?;
    if tail.len() != count.checked_add(1)? {
        return None;
    }
    let mut messages = Vec::with_capacity(count);
    let mut seen = BTreeSet::new();
    for field in tail.iter().skip(1) {
        let entry = split_1c_braced_fields(field.trim(), 0)?;
        if entry.len() != 2 {
            return None;
        }
        let id = entry.first()?.trim().parse::<u32>().ok()?;
        if !seen.insert(id) {
            return None;
        }
        messages.push(ConfigurationMobileApplicationPermissionMessage {
            permission: configuration_mobile_application_permission_name(id)?,
            description: parse_1c_synonyms(entry.get(1)?.trim()),
        });
    }
    Some(messages)
}

fn configuration_root_property_fields<'a>(text: &'a str, uuid: &str) -> Option<Vec<&'a str>> {
    parse_configuration_root_layout(text, uuid)?;
    let envelope = parse_configuration_root_envelope(text)?;
    // Both footer variants reach this 60/61/77-length tuple shape and feed
    // `configuration_properties_evidenced_default_block_policy`'s "matches
    // the evidenced all-default reference outside six proven bytes" claim.
    // WMS5's `МодульWebОбмена_ERP25.cf` (Bare footer) proved that claim can
    // hold for Bare data too, once genuinely new evidence is accounted for:
    // its `InterfaceCompatibilityMode` byte (`'3'`) was a not-yet-seen enum
    // member (now mapped to `Taxi`, matching its native `Configuration.xml`),
    // and once that was fixed, every other verbatim-compared field matched
    // the reference exactly. So the earlier Checksummed-only restriction was
    // evidence the *policy* wasn't ready yet, not evidence the footer
    // variant itself mattered -- `envelope.footer` needs no check here
    // beyond what `parse_configuration_root_layout` already required.
    let first_section = envelope.sections.first()?.trim();
    let contained_fields = split_1c_braced_fields(first_section, 0)?;
    if contained_fields.len() != 2 {
        return None;
    }
    let payload_fields = split_1c_braced_fields(contained_fields.get(1)?.trim(), 0)?;
    if payload_fields.first().map(|field| field.trim()) != Some("1") {
        return None;
    }
    let fields = split_1c_braced_fields(payload_fields.get(1)?.trim(), 0)?;
    match (fields.first()?.trim(), fields.len()) {
        ("67", 60) => {}
        ("68", 61) if fields.get(60)?.trim() == "1" => {}
        ("76", 77) => {}
        _ => return None,
    }
    let mut object_ids = configuration_contained_object_ids(first_section).into_iter();
    let object_id = object_ids.next()?;
    if object_ids.next().is_some() {
        return None;
    }
    if !is_configuration_root_property_header(fields.get(1)?.trim(), &object_id) {
        return None;
    }
    // Normalize the short `{67,...}` shape to the canonical `{68,...}` one
    // (see `configuration_root_fields`'s doc comment for the evidence) so
    // `parse_configuration_properties_evidenced_default_block`'s exact-arity
    // check against the 61-field evidenced reference sees this corpus's real
    // shape instead of always refusing it as `UnexpectedTupleArity`.
    normalize_short_configuration_root_property_fields(fields)
}

fn is_configuration_root_property_header(field: &str, object_id: &str) -> bool {
    let Some(wrapper) = split_1c_braced_fields(field, 0) else {
        return false;
    };
    if wrapper.len() != 2 || wrapper.first().map(|field| field.trim()) != Some("0") {
        return false;
    }
    let Some(header) = wrapper
        .get(1)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))
    else {
        return false;
    };
    // Same short-wrapper omission `parse_information_register_owner_header`
    // (`0575505`) and `innermost_metadata_object_fields_around_header`
    // (this pass) document for this exact generic
    // `{1,0,<uuid>},Name,Synonym,Comment,0,0,NilUuid,0` header shape: the
    // platform drops the trailing default `0` (and the wrapper's own
    // leading count from `3` to `2`) whenever the Configuration root's own
    // header leaves that slot at default. One object per config, so a
    // small blast radius, but a total-parse failure if hit (the whole
    // Configuration.xml's default roles/use-purposes/localized properties
    // read depends on this header resolving).
    let header_has_trailing_default = match header.len() {
        9 => true,
        8 => false,
        _ => return false,
    };
    if header.first().map(|field| field.trim())
        != Some(if header_has_trailing_default {
            "3"
        } else {
            "2"
        })
        || header
            .get(2)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .is_none()
        || !is_configuration_root_synonym_field(header.get(3).copied())
        || header
            .get(4)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .is_none()
        || header.get(5).map(|field| field.trim()) != Some("0")
        || header.get(6).map(|field| field.trim()) != Some("0")
        || header.get(7).map(|field| field.trim()) != Some("00000000-0000-0000-0000-000000000000")
        || (header_has_trailing_default && header.get(8).map(|field| field.trim()) != Some("0"))
    {
        return false;
    }
    let Some(identity) = header
        .get(1)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))
    else {
        return false;
    };
    identity.len() == 3
        && identity.first().map(|field| field.trim()) == Some("1")
        && identity.get(1).map(|field| field.trim()) == Some("0")
        && identity.get(2).map(|field| field.trim()) == Some(object_id)
}

fn is_configuration_root_synonym_field(field: Option<&str>) -> bool {
    let Some(fields) = field.and_then(|field| split_1c_braced_fields(field.trim(), 0)) else {
        return false;
    };
    let Some(count) = fields
        .first()
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return false;
    };
    let Some(expected_len) = count.checked_mul(2).and_then(|value| value.checked_add(1)) else {
        return false;
    };
    fields.len() == expected_len
        && fields
            .iter()
            .skip(1)
            .all(|field| parse_1c_quoted_string(field.trim()).is_some())
}

const CONFIGURATION_CONTAINED_OBJECT_COUNT: usize = 7;

const CONFIGURATION_ROOT_CHILD_KIND_ORDER: [&str; 45] = [
    "Language",
    "Subsystem",
    "StyleItem",
    "Style",
    "CommonPicture",
    "SessionParameter",
    "Role",
    "CommonTemplate",
    "FilterCriterion",
    "CommonModule",
    "CommonAttribute",
    "ExchangePlan",
    "XDTOPackage",
    "WebService",
    "HTTPService",
    "WSReference",
    "EventSubscription",
    "ScheduledJob",
    "SettingsStorage",
    "FunctionalOption",
    "FunctionalOptionsParameter",
    "DefinedType",
    "Bot",
    "CommonCommand",
    "CommandGroup",
    "Constant",
    "CommonForm",
    "Catalog",
    "Document",
    "DocumentNumerator",
    "Sequence",
    "DocumentJournal",
    "Enum",
    "Report",
    "DataProcessor",
    "InformationRegister",
    "AccumulationRegister",
    "ChartOfCharacteristicTypes",
    "ChartOfAccounts",
    "AccountingRegister",
    "ChartOfCalculationTypes",
    "CalculationRegister",
    "BusinessProcess",
    "Task",
    "IntegrationService",
];

/// The evidenced Configuration root envelope shared by every root consumer:
/// `{2,{Identity},N,<section 1>...<section N>,{footer}}` — a flat field list
/// whose declared section count must match the actual slots exactly (see
/// `docs/evidence/configuration-body-8.3.27.md` and the retained CF corpora
/// under `tests/fixtures/native-evidence/8.3.27.2214`). This parser is the
/// single source of truth for the outer root shape; consumers layer their own
/// stricter requirements (section count, footer variant) on top of it instead
/// of re-deriving the slot arithmetic.
struct ConfigurationRootEnvelope<'a> {
    identity: String,
    sections: Vec<&'a str>,
    footer: ConfigurationRootFooter,
}

fn parse_configuration_root_envelope(text: &str) -> Option<ConfigurationRootEnvelope<'_>> {
    let root = split_1c_braced_fields(text.trim_start(), 0)?;
    if root.first()?.trim() != "2" {
        return None;
    }
    let identity_fields = split_1c_braced_fields(root.get(1)?.trim(), 0)?;
    if identity_fields.len() != 1 {
        return None;
    }
    let identity = parse_non_zero_uuid(identity_fields.first()?.trim())?;
    let section_count = root.get(2)?.trim().parse::<usize>().ok()?;
    if section_count == 0 || section_count.checked_add(4)? != root.len() {
        return None;
    }
    let footer = classify_configuration_root_footer(root.last()?.trim())?;
    Some(ConfigurationRootEnvelope {
        identity,
        sections: root[3..3 + section_count].to_vec(),
        footer,
    })
}

fn parse_configuration_root_layout(text: &str, uuid: &str) -> Option<ConfigurationRootLayout> {
    let envelope = parse_configuration_root_envelope(text)?;
    // Both evidenced footer variants carry the identical
    // `{ClassId,{1,{payload}}}` section shape the loop below decodes.
    // Confirmed against ERP УХ's `Web_Service.cf` and WMS5's
    // `МодульWebОбмена_ERP25.cf`: both carry a `{{0,"",""}}` (Bare) root
    // footer, and both native reference trees still emit the full
    // `InternalInfo`/`ChildObjects` shape for their Configuration.xml --
    // requiring `Checksummed` here silently dropped ~2/3 of Configuration.xml
    // (no InternalInfo, no ChildObjects, ~15 Properties fields) for every
    // configuration whose root record happens to carry the Bare footer.
    // Any *other* footer shape already fails closed inside
    // `classify_configuration_root_footer`, which built this envelope.
    if envelope.identity != uuid
        || envelope.sections.len() != CONFIGURATION_CONTAINED_OBJECT_COUNT
        || !matches!(
            envelope.footer,
            ConfigurationRootFooter::Bare | ConfigurationRootFooter::Checksummed
        )
    {
        return None;
    }

    let mut contained_objects = Vec::with_capacity(envelope.sections.len());
    let mut child_families = Vec::new();
    for field in &envelope.sections {
        let contained_fields = split_1c_braced_fields(field.trim(), 0)?;
        if contained_fields.len() != 2 {
            return None;
        }
        let class_id = parse_non_zero_uuid(contained_fields.first()?.trim())?;
        let object_ids = configuration_contained_object_ids(field);
        if object_ids.len() != 1 {
            return None;
        }
        let object_id = object_ids.into_iter().next()?;
        let families = configuration_family_sequence(contained_fields.get(1)?.trim(), &object_id)?;
        contained_objects.push(ConfigurationContainedObject {
            class_id,
            object_id,
        });
        child_families.extend(families);
    }

    Some(ConfigurationRootLayout {
        contained_objects,
        child_families,
    })
}

/// The two evidenced Configuration root-control tails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfigurationRootFooter {
    /// `{{0,"",""}}` — the db-resident tail recorded for the 8.3.27.1989
    /// cohort (`docs/evidence/configuration-body-8.3.27.md`) and emitted by
    /// the clean-room bootstrap writer (`compiler::root`).
    Bare,
    /// `{{1,"",""},{checksum}}` — a `{1,"",""}` marker followed by a signed
    /// 32-bit body checksum, e.g. `{{1,"",""},{-1648891888}}` (see
    /// `tests/fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid`,
    /// confirmed identically across all 19 retained CF corpora). The checksum
    /// value itself is opaque and is not interpreted or re-derived here.
    Checksummed,
}

fn classify_configuration_root_footer(field: &str) -> Option<ConfigurationRootFooter> {
    let fields = split_1c_braced_fields(field, 0)?;
    let marker = split_1c_braced_fields(fields.first()?.trim(), 0)?;
    let marker_tail_valid = marker.len() == 3
        && marker
            .get(1)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .is_some_and(|value| value.is_empty())
        && marker
            .get(2)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .is_some_and(|value| value.is_empty());
    if !marker_tail_valid {
        return None;
    }
    match (marker.first()?.trim(), fields.len()) {
        ("0", 1) => Some(ConfigurationRootFooter::Bare),
        ("1", 2) => {
            let checksum = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
            (checksum.len() == 1
                && checksum
                    .first()
                    .is_some_and(|value| value.trim().parse::<i32>().is_ok()))
            .then_some(ConfigurationRootFooter::Checksummed)
        }
        _ => None,
    }
}

fn configuration_contained_object_ids(text: &str) -> Vec<String> {
    const MARKER: &str = "{1,0,";

    let mut object_ids = Vec::new();
    let mut search_start = 0;
    while let Some(relative_start) = text[search_start..].find(MARKER) {
        let marker_start = search_start + relative_start;
        search_start = marker_start + 1;
        let Some(fields) = split_1c_braced_fields(text, marker_start) else {
            continue;
        };
        if fields.len() != 3
            || fields.first().map(|field| field.trim()) != Some("1")
            || fields.get(1).map(|field| field.trim()) != Some("0")
        {
            continue;
        }
        if let Some(object_id) = fields
            .get(2)
            .and_then(|field| parse_non_zero_uuid(field.trim()))
        {
            object_ids.push(object_id);
        }
    }
    object_ids
}

fn configuration_family_sequence(text: &str, object_id: &str) -> Option<Vec<Vec<String>>> {
    let mut candidates = Vec::new();
    collect_configuration_family_sequences(text, object_id, 0, &mut candidates);
    let minimum_depth = candidates.iter().map(|(depth, _)| *depth).min()?;
    let mut nearest = candidates
        .into_iter()
        .filter(|(depth, _)| *depth == minimum_depth)
        .map(|(_, families)| families);
    let families = nearest.next()?;
    nearest.next().is_none().then_some(families)
}

fn collect_configuration_family_sequences(
    text: &str,
    object_id: &str,
    depth: usize,
    candidates: &mut Vec<(usize, Vec<Vec<String>>)>,
) {
    let Some(fields) = split_1c_braced_fields(text, 0) else {
        return;
    };
    let initial_candidate_count = candidates.len();

    for count_index in 1..fields.len() {
        let Some(family_count) = fields
            .get(count_index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if family_count == 0 || fields.len() != count_index + family_count + 1 {
            continue;
        }
        if !fields[..count_index].iter().any(|field| {
            configuration_contained_object_ids(field)
                .iter()
                .any(|candidate| candidate == object_id)
        }) {
            continue;
        }

        let mut families = Vec::with_capacity(family_count);
        let mut valid = true;
        for family in fields.iter().skip(count_index + 1) {
            let Some(children) = parse_configuration_family(family.trim()) else {
                valid = false;
                break;
            };
            families.push(children);
        }
        if valid {
            candidates.push((depth, families));
        }
    }

    if candidates.len() != initial_candidate_count {
        return;
    }

    for field in &fields {
        let field = field.trim();
        if field.starts_with('{') {
            collect_configuration_family_sequences(field, object_id, depth + 1, candidates);
        }
    }
}

fn parse_configuration_family(text: &str) -> Option<Vec<String>> {
    let fields = split_1c_braced_fields(text, 0)?;
    parse_non_zero_uuid(fields.first()?.trim())?;
    let child_count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if fields.len() != child_count + 2 {
        return None;
    }
    fields
        .iter()
        .skip(2)
        .map(|field| parse_non_zero_uuid(field.trim()))
        .collect()
}

fn resolve_configuration_root_child_objects(
    layout: &ConfigurationRootLayout,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<ConfigurationRootChildObject>> {
    let mut seen_uuids = BTreeSet::new();
    let mut seen_kinds = BTreeSet::new();
    let mut groups = Vec::new();

    for family in &layout.child_families {
        if family.is_empty() {
            continue;
        }
        let mut family_kind = None;
        let mut names = Vec::with_capacity(family.len());
        for uuid in family {
            if !seen_uuids.insert(uuid.as_str()) {
                return None;
            }
            let Some(reference) = object_refs.get(uuid) else {
                return None;
            };
            let Some((kind, name)) = reference.split_once('.') else {
                return None;
            };
            if name.is_empty() || name.contains('.') {
                return None;
            }
            let Some((order, kind)) = configuration_root_child_kind(kind) else {
                return None;
            };
            if family_kind.is_some_and(|(_, candidate)| candidate != kind) {
                return None;
            }
            family_kind = Some((order, kind));
            names.push(name.to_string());
        }
        let (order, kind) = family_kind?;
        if !seen_kinds.insert(kind) {
            return None;
        }
        groups.push((order, kind, names));
    }

    groups.sort_by_key(|(order, _, _)| *order);
    Some(
        groups
            .into_iter()
            .flat_map(|(_, kind, names)| {
                names
                    .into_iter()
                    .map(move |name| ConfigurationRootChildObject { kind, name })
            })
            .collect(),
    )
}

fn configuration_root_child_kind(kind: &str) -> Option<(usize, &'static str)> {
    CONFIGURATION_ROOT_CHILD_KIND_ORDER
        .iter()
        .enumerate()
        .find(|(_, candidate)| **candidate == kind)
        .map(|(order, kind)| (order, *kind))
}

pub(super) fn parse_configuration_localized_property(
    fields: &[&str],
    index: usize,
) -> Vec<(String, String)> {
    fields
        .get(index)
        .map(|field| parse_1c_synonyms(field))
        .unwrap_or_default()
}

pub(super) fn parse_configuration_root_reference(
    fields: &[&str],
    index: usize,
    object_refs: &BTreeMap<String, String>,
    expected_prefix: &str,
) -> Option<String> {
    parse_configuration_root_reference_slot(fields, index, object_refs, expected_prefix)?.value
}

pub(super) fn parse_configuration_root_reference_slot(
    fields: &[&str],
    index: usize,
    object_refs: &BTreeMap<String, String>,
    expected_prefix: &str,
) -> Option<ConfigurationRootReference> {
    let field = fields.get(index)?.trim();
    if field == "00000000-0000-0000-0000-000000000000" {
        return Some(ConfigurationRootReference { value: None });
    }
    let uuid = parse_non_zero_uuid(field)?;
    let reference = object_refs.get(&uuid)?;
    reference
        .starts_with(expected_prefix)
        .then(|| ConfigurationRootReference {
            value: Some(reference.clone()),
        })
}

pub(super) fn configuration_default_run_mode_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("OrdinaryApplication"),
        "1" => Some("ManagedApplication"),
        _ => None,
    }
}

pub(super) fn configuration_script_variant_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Russian"),
        "1" => Some("English"),
        _ => None,
    }
}

/// The highest packed platform-version value this reader has direct
/// evidence for: `80327` = `Version8_3_27`, this platform's own build
/// (`ibcmd` 8.3.27.2214). WMS5's `МодульWebОбмена_ERP25.cf` carries `80501`
/// at the `CompatibilityMode` coordinate (tuple field 43) while its native
/// `Configuration.xml` prints `Version8_3_27` -- the same value this
/// platform's own version formats to, not `Version8_5_1`. A config cannot
/// prove compatibility with a platform edition newer than the one reading
/// it, so a value above this ceiling is evidence of a still-unproven
/// spelling (e.g. a not-yet-observed clamping/rounding rule), not proof
/// that the naive `major*10000+minor*100+patch` unpacking is the right
/// reading for it. Fail closed rather than print the arithmetic guess.
const MAX_EVIDENCED_PACKED_PLATFORM_VERSION: u32 = 80327;

pub(super) fn configuration_compatibility_mode_xml(value: &str) -> Option<String> {
    if let Some(value) = parse_1c_quoted_string(value) {
        return if value.is_empty() { None } else { Some(value) };
    }
    let version = value.parse::<u32>().ok()?;
    if version < 80000 || version > MAX_EVIDENCED_PACKED_PLATFORM_VERSION {
        return None;
    }
    Some(format!(
        "Version{}_{}_{}",
        version / 10000,
        (version / 100) % 100,
        version % 100
    ))
}

struct ConfigurationChildObject {
    tag: &'static str,
    header: MetadataHeader,
}

fn parse_configuration_child_objects(
    text: &str,
    uuid: &str,
    header_uuid: &str,
) -> Vec<ConfigurationChildObject> {
    nested_headers_with_offsets_from_text(text, uuid, |_| true)
        .into_iter()
        .filter_map(|(header, marker_start)| {
            if header.uuid == header_uuid {
                return None;
            }
            configuration_child_object_tag(text, marker_start, &header.uuid)
                .map(|tag| ConfigurationChildObject { tag, header })
        })
        .collect()
}

pub(super) fn configuration_child_object_tag(
    text: &str,
    marker_start: usize,
    child_uuid: &str,
) -> Option<&'static str> {
    let mut search_end = marker_start;
    let mut tag = None;
    while let Some(start) = text[..search_end].rfind('{') {
        search_end = start;
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        if marker_start >= end {
            continue;
        }
        let object_text = &text[start..end];
        if object_text.contains("{68,") {
            continue;
        }
        let Some(fields) = split_1c_braced_fields(object_text, 0) else {
            continue;
        };
        let Some(code) = fields
            .first()
            .and_then(|field| field.trim().parse::<u32>().ok())
        else {
            continue;
        };
        if let Some((kind, _)) = metadata_source_for_object_text(code, object_text, child_uuid) {
            if matches!(kind, "CommonForm" | "CommonTemplate") {
                return None;
            }
            tag = Some(kind);
        }
    }
    tag
}

#[allow(dead_code)]
pub(super) fn build_command_interface_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, String> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_command_interface_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_command_interface_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, String> {
    let row_entries = parallel::install(|| {
        rows.par_iter()
            .enumerate()
            .map(|(index, row)| (index, command_interface_reference_entries_from_text(row)))
            .collect::<Vec<_>>()
    })
    .unwrap_or_else(|_| {
        rows.iter()
            .enumerate()
            .map(|(index, row)| (index, command_interface_reference_entries_from_text(row)))
            .collect::<Vec<_>>()
    });
    let mut index = BTreeMap::new();
    for (_, entries) in row_entries {
        for (uuid, reference) in entries {
            index.insert(uuid, reference);
        }
    }
    index
}

pub(super) fn command_interface_reference_entries_from_text(
    row: &MetadataTextRow,
) -> Vec<(String, String)> {
    let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    if kind == "CommonCommand" {
        entries.push((
            row.file_name.clone(),
            format!("CommonCommand.{}", header.name),
        ));
    }
    entries.extend(
        nested_command_headers_for_owner_from_text(kind, &row.text, &row.file_name)
            .into_iter()
            .map(|command| {
                (
                    command.uuid,
                    format!("{}.{}.Command.{}", kind, header.name, command.name),
                )
            }),
    );
    entries
}

#[allow(dead_code)]
pub(super) fn parse_metadata_command_reference_blob(
    blob: &[u8],
    uuid: &str,
) -> Option<(String, MetadataHeader, String)> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}').to_string();
    let object_code = parse_metadata_object_code(&text)?;
    let kind = if object_code == 12 {
        "CommonModule"
    } else {
        metadata_source_for_text(object_code, &text, uuid)?.0
    };
    let header = parse_metadata_header_from_text(&text, uuid)?;
    Some((kind.to_string(), header, text))
}
