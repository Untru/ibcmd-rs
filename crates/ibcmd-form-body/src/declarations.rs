//! What the configuration's metadata declares for a dynamic list's
//! resolvable-field universe and a constants set's `<UseAlways>`, read once
//! per export by the root's `build_metadata_field_declaration_index_from_texts`
//! (`src/mssql_dump/refs.rs`, where these types were declared).

use std::collections::{BTreeMap, BTreeSet};

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
pub struct MetadataTableStandardAttributes {
    pub code_length: Option<u32>,
    pub description_length: Option<u32>,
    /// Whether `<Owners>` declares at least one owner.
    pub owned: Option<bool>,
    pub hierarchical: Option<bool>,
    /// Whether `<HierarchyType>` is `HierarchyFoldersAndItems`.
    pub folder_hierarchy: Option<bool>,
    /// Whether `<InformationRegisterPeriodicity>` is anything but
    /// `Nonperiodical`.
    pub periodical: Option<bool>,
}

impl MetadataTableStandardAttributes {
    /// Whether the table declares the standard attribute its family spells
    /// `english`. A name this index knows no property for is declared.
    pub fn declares(&self, english: &str) -> bool {
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
pub struct MetadataCommonAttributeContent {
    pub auto_use: bool,
    pub content: BTreeMap<String, bool>,
}

impl MetadataCommonAttributeContent {
    pub fn covers(&self, table: &str) -> bool {
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
pub struct MetadataConstantDeclaration {
    pub always_used: bool,
    pub value_storage: bool,
}

/// The declarations a dynamic list's resolvable-field universe is built from.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct MetadataFieldDeclarationIndex {
    /// Whether a manual-query dynamic list leaves the main register's
    /// dimensions out of its automatic fields: under compatibility 8.3.17
    /// (Документооборот 3.0, ERP WE 2.5) the platform marks an unselected
    /// dimension `~`, under 8.3.24 (Документооборот 3.0 Холдинг) it does
    /// not. The boundary between is unobserved; the 8.3.19 boundary the
    /// default-picture marker moves at is taken for it.
    register_dimensions_withheld: bool,
    pub tables: BTreeMap<String, MetadataTableStandardAttributes>,
    /// Top-level data fields declared by each metadata table, folded to lower
    /// case. The object-reference index supplies these names independently of
    /// any query text, so query validation never infers declarations from use.
    pub data_fields: BTreeMap<String, BTreeSet<String>>,
    /// The owner a top-level data field's *single declared reference type*
    /// names, keyed by the owning table and the folded field name. A field
    /// that declares no type, several, or a non-reference one is absent: a
    /// value with more than one possible type has no single set of fields to
    /// dereference into.
    pub field_types: BTreeMap<String, BTreeMap<String, String>>,
    /// Declared fields whose metadata property enables password mode, keyed by
    /// their owning table and folded field name. Password fields are excluded
    /// from a dynamic list's automatically available fields by the platform.
    pub password_fields: BTreeMap<String, BTreeSet<String>>,
    pub common_attributes: BTreeMap<String, MetadataCommonAttributeContent>,
    /// Every constant the configuration declares, by its own uuid -- the id a
    /// form's use-always record spells.
    pub constants: BTreeMap<String, MetadataConstantDeclaration>,
    /// Every `Kind.Name` the configuration declares, folded to lower case: the
    /// query language names metadata case-insensitively, so a query naming
    /// `Перечисление.СтатусызаданийТорговымПредставителям` names the declared
    /// `Enum.СтатусыЗаданийТорговымПредставителям`.
    pub declared_tables: BTreeSet<String>,
    /// Documents whose references form a document journal's heterogeneous
    /// `Ref` value. A member behind that value is available only when every
    /// registered document declares it.
    pub document_journal_documents: BTreeMap<String, BTreeSet<String>>,
    /// Whether a constants set's `<UseAlways>` names its `v8:ValueStorage`
    /// constants: 2.21 does (8.5.1.1150 ERP УХ, 75 forms), 2.20 never does.
    pub writes_value_storage_constants: bool,
    /// Whether the configuration's script variant is English (root field 3
    /// `0`): its queries name standard attributes in English only, so a
    /// Russian standard name is a field the list cannot resolve (ERP WE
    /// English `Catalogs/ItemKinds/Forms/ListFormForPricingSettings` is
    /// written `~List.Наименование`).
    pub english_script: bool,
}

impl MetadataFieldDeclarationIndex {
    /// The index as the writer of `source_version` reads it.
    pub fn withholding_register_dimensions(mut self, withheld: bool) -> Self {
        self.register_dimensions_withheld = withheld;
        self
    }

    pub fn register_dimensions_withheld(&self) -> bool {
        self.register_dimensions_withheld
    }

    pub fn writes_value_storage_constants(&self) -> bool {
        self.writes_value_storage_constants
    }

    pub fn english_script(&self) -> bool {
        self.english_script
    }

    pub fn table(&self, reference: &str) -> Option<&MetadataTableStandardAttributes> {
        self.tables.get(reference)
    }

    pub fn common_attribute(&self, name: &str) -> Option<&MetadataCommonAttributeContent> {
        self.common_attributes.get(name)
    }

    pub fn field_uses_password_mode(&self, table: &str, field: &str) -> bool {
        self.password_fields
            .get(table)
            .is_some_and(|fields| fields.contains(&field.to_lowercase()))
    }

    pub fn declares_data_field(&self, table: &str, field: &str) -> Option<bool> {
        self.data_fields
            .get(table)
            .map(|fields| fields.contains(&field.to_lowercase()))
    }

    /// The metadata object a field of `table` holds a reference to, when the
    /// field declares exactly one reference type and nothing else.
    pub fn field_reference_owner(&self, table: &str, field: &str) -> Option<&str> {
        self.field_types
            .get(table)?
            .get(&field.to_lowercase())
            .map(String::as_str)
    }

    /// What the constant with this uuid declares, or `None` when this index
    /// carries no constant declarations at all -- a refusal to answer, not a
    /// denial, so a context built without them keeps writing exactly the
    /// record.
    pub fn constant(&self, uuid: &str) -> Option<&MetadataConstantDeclaration> {
        self.constants.get(uuid)
    }

    /// The constants this configuration declares always used, by uuid. Empty
    /// when the index carries no constant declarations.
    pub fn always_used_constants(&self) -> impl Iterator<Item = &str> {
        self.constants
            .iter()
            .filter(|(_, declared)| declared.always_used)
            .map(|(uuid, _)| uuid.as_str())
    }

    /// Whether the configuration declares a metadata table by this
    /// `Kind.Name`. `None` when this index carries no table names at all, which
    /// is a refusal to answer rather than a denial.
    pub fn declares_table(&self, reference: &str) -> Option<bool> {
        if self.declared_tables.is_empty() {
            return None;
        }
        Some(self.declared_tables.contains(&reference.to_lowercase()))
    }

    pub fn document_journal_documents(&self, reference: &str) -> Option<&BTreeSet<String>> {
        self.document_journal_documents.get(reference)
    }
}
