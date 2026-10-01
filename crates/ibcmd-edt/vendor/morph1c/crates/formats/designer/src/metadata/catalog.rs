//! Designer-проекция вида `Catalog` + его дочерних видов (зеркало
//! `core/spec/metadata/catalog*.rs`, ARCHITECTURE.md §5; child-objects substrate). ТОЛЬКО
//! размещение/кодировка — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5), включая детей.
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<Catalog>/<Properties>`, DENSE. Дети
//! Attribute/TabularSection/Command — `<ChildObjects>`+`<Properties>` (props_wrapped=true);
//! TabularSection — recursion-узел (InternalInfo+nested Attribute). Forms/Templates —
//! BARE-ссылки `<Form>Имя</Form>`. Designer DENSE: эмитит и дефолты; порядок = спек-порядок.

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::catalog_child as base;
use morph1c_core::spec::metadata::catalog as cat;
use morph1c_core::spec::metadata::catalog::catalog;
use morph1c_core::spec::metadata::catalog_attribute::catalog_attribute;
use morph1c_core::spec::metadata::catalog_command as cmd;
use morph1c_core::spec::metadata::catalog_command::catalog_command;
use morph1c_core::spec::metadata::catalog_form_ref::catalog_form_ref;
use morph1c_core::spec::metadata::catalog_tabular_section as ts;
use morph1c_core::spec::metadata::catalog_tabular_section::catalog_tabular_section;
use morph1c_core::spec::metadata::catalog_tabular_section_attribute::catalog_tabular_section_attribute;
use morph1c_core::spec::metadata::catalog_template_ref::catalog_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

// Пути от корня <MetaDataObject> через <Catalog>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["Catalog", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerCatalog;

impl LocusMap for DesignerCatalog {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cat::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            cat::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            cat::F_HIERARCHICAL => fp(p!("Hierarchical"), Codec::BoolText),
            cat::F_HIERARCHY_TYPE => fp(p!("HierarchyType"), Codec::EnumText),
            cat::F_LIMIT_LEVEL_COUNT => fp(p!("LimitLevelCount"), Codec::BoolText),
            cat::F_LEVEL_COUNT => fp(p!("LevelCount"), Codec::IntText),
            cat::F_FOLDERS_ON_TOP => fp(p!("FoldersOnTop"), Codec::BoolText),
            cat::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            cat::F_OWNERS => fp(p!("Owners"), Codec::RefList(RefListDialect::DesignerItem)),
            cat::F_SUBORDINATION_USE => fp(p!("SubordinationUse"), Codec::EnumText),
            cat::F_CODE_LENGTH => fp(p!("CodeLength"), Codec::IntText),
            cat::F_DESCRIPTION_LENGTH => fp(p!("DescriptionLength"), Codec::IntText),
            cat::F_CODE_TYPE => fp(p!("CodeType"), Codec::EnumText),
            cat::F_CODE_ALLOWED_LENGTH => fp(p!("CodeAllowedLength"), Codec::EnumText),
            cat::F_CODE_SERIES => fp(p!("CodeSeries"), Codec::EnumText),
            cat::F_CHECK_UNIQUE => fp(p!("CheckUnique"), Codec::BoolText),
            cat::F_AUTONUMBERING => fp(p!("Autonumbering"), Codec::BoolText),
            cat::F_DEFAULT_PRESENTATION => fp(p!("DefaultPresentation"), Codec::EnumText),
            cat::F_STANDARD_ATTRIBUTES => fp(
                p!("StandardAttributes"),
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &cat::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            cat::F_CHARACTERISTICS => fp(p!("Characteristics"), Codec::Characteristics(CharacteristicsDialect::Designer)),
            cat::F_PREDEFINED_DATA_UPDATE => fp(p!("PredefinedDataUpdate"), Codec::EnumText),
            cat::F_EDIT_TYPE => fp(p!("EditType"), Codec::EnumText),
            cat::F_QUICK_CHOICE => fp(p!("QuickChoice"), Codec::BoolText),
            cat::F_CHOICE_MODE => fp(p!("ChoiceMode"), Codec::EnumText),
            cat::F_INPUT_BY_STRING => fp(p!("InputByString"), Codec::RefList(RefListDialect::DesignerField)),
            cat::F_SEARCH_STRING_MODE => fp(p!("SearchStringModeOnInputByString"), Codec::EnumText),
            cat::F_FTS_ON_INPUT => fp(p!("FullTextSearchOnInputByString"), Codec::EnumText),
            cat::F_CHOICE_DATA_GET_MODE => fp(p!("ChoiceDataGetModeOnInputByString"), Codec::EnumText),
            cat::F_DEFAULT_OBJECT_FORM => fp(p!("DefaultObjectForm"), Codec::PlainText),
            cat::F_DEFAULT_FOLDER_FORM => fp(p!("DefaultFolderForm"), Codec::PlainText),
            cat::F_DEFAULT_LIST_FORM => fp(p!("DefaultListForm"), Codec::PlainText),
            cat::F_DEFAULT_CHOICE_FORM => fp(p!("DefaultChoiceForm"), Codec::PlainText),
            cat::F_DEFAULT_FOLDER_CHOICE_FORM => fp(p!("DefaultFolderChoiceForm"), Codec::PlainText),
            cat::F_AUX_OBJECT_FORM => fp(p!("AuxiliaryObjectForm"), Codec::PlainText),
            cat::F_AUX_FOLDER_FORM => fp(p!("AuxiliaryFolderForm"), Codec::PlainText),
            cat::F_AUX_LIST_FORM => fp(p!("AuxiliaryListForm"), Codec::PlainText),
            cat::F_AUX_CHOICE_FORM => fp(p!("AuxiliaryChoiceForm"), Codec::PlainText),
            cat::F_AUX_FOLDER_CHOICE_FORM => fp(p!("AuxiliaryFolderChoiceForm"), Codec::PlainText),
            cat::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            // help — EDT-only (Designer не несёт): not projected.
            cat::F_BASED_ON => fp(p!("BasedOn"), Codec::RefList(RefListDialect::DesignerItem)),
            cat::F_DATA_LOCK_FIELDS => fp(p!("DataLockFields"), Codec::RefList(RefListDialect::DesignerField)),
            cat::F_DATA_LOCK_CONTROL_MODE => fp(p!("DataLockControlMode"), Codec::EnumText),
            cat::F_FULL_TEXT_SEARCH => fp(p!("FullTextSearch"), Codec::EnumText),
            cat::F_OBJECT_PRESENTATION => fp(p!("ObjectPresentation"), Codec::LocalizedV8),
            cat::F_EXTENDED_OBJECT_PRESENTATION => fp(p!("ExtendedObjectPresentation"), Codec::LocalizedV8),
            cat::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            cat::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            cat::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            cat::F_CREATE_ON_INPUT => fp(p!("CreateOnInput"), Codec::EnumText),
            cat::F_CHOICE_HISTORY_ON_INPUT => fp(p!("ChoiceHistoryOnInput"), Codec::EnumText),
            cat::F_DATA_HISTORY => fp(p!("DataHistory"), Codec::EnumText),
            cat::F_UPDATE_DATA_HISTORY => fp(p!("UpdateDataHistoryImmediatelyAfterWrite"), Codec::BoolText),
            cat::F_EXECUTE_AFTER_WRITE_DH => fp(p!("ExecuteAfterWriteDataHistoryVersionProcessing"), Codec::BoolText),
            // predefined — EDT-only (Designer не несёт inline): not projected.
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["Catalog", "ChildObjects"];
        let (tag, bare) = match collection {
            "Attribute" => ("Attribute", false),
            "TabularSection" => ("TabularSection", false),
            "Command" => ("Command", false),
            "Form" => ("Form", true),
            "Template" => ("Template", true),
            _ => return None,
        };
        Some(ChildLocus { container: cont, child_tag: tag, props_wrapped: !bare, name_tag: "Name", bare_ref: bare })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        cat_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // ВСЕ 74 Designer-Catalog несут `<ChildObjects>` (1 — пустой при нуле детей).
        true
    }
}

// ===== Дети Attribute / TabularSection.Attribute (Catalog base) =====
fn base_lookup(field: FieldId) -> Option<FieldProjection> {
    Some(match field {
        irc::F_SYNONYM => fp(&["Synonym"], Codec::LocalizedV8),
        irc::F_COMMENT => fp(&["Comment"], Codec::PlainText),
        irc::F_TYPE => fp(&["Type"], Codec::Type(formats_xml::TypeDialect::Designer)),
        irc::F_PASSWORD_MODE => fp(&["PasswordMode"], Codec::BoolText),
        irc::F_FORMAT => fp(&["Format"], Codec::LocalizedV8),
        irc::F_EDIT_FORMAT => fp(&["EditFormat"], Codec::LocalizedV8),
        irc::F_TOOL_TIP => fp(&["ToolTip"], Codec::LocalizedV8),
        irc::F_MARK_NEGATIVES => fp(&["MarkNegatives"], Codec::BoolText),
        irc::F_MASK => fp(&["Mask"], Codec::PlainText),
        irc::F_MULTI_LINE => fp(&["MultiLine"], Codec::BoolText),
        irc::F_EXTENDED_EDIT => fp(&["ExtendedEdit"], Codec::BoolText),
        irc::F_MIN_VALUE => fp(&["MinValue"], Codec::Value(ValueDialect::Designer)),
        irc::F_MAX_VALUE => fp(&["MaxValue"], Codec::Value(ValueDialect::Designer)),
        irc::F_FILL_FROM_FILLING_VALUE => fp(&["FillFromFillingValue"], Codec::BoolText),
        irc::F_FILL_VALUE => fp(&["FillValue"], Codec::Value(ValueDialect::Designer)),
        irc::F_FILL_CHECKING => fp(&["FillChecking"], Codec::EnumText),
        irc::F_CHOICE_FOLDERS_AND_ITEMS => fp(&["ChoiceFoldersAndItems"], Codec::EnumText),
        irc::F_CHOICE_PARAMETER_LINKS => fp(
            &["ChoiceParameterLinks"],
            Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Designer),
        ),
        irc::F_CHOICE_PARAMETERS => fp(
            &["ChoiceParameters"],
            Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Designer),
        ),
        irc::F_QUICK_CHOICE => fp(&["QuickChoice"], Codec::EnumText),
        irc::F_CREATE_ON_INPUT => fp(&["CreateOnInput"], Codec::EnumText),
        irc::F_CHOICE_FORM => fp(&["ChoiceForm"], Codec::PlainText),
        irc::F_LINK_BY_TYPE => fp(&["LinkByType"], Codec::LinkByType(formats_xml::link_by_type::LinkByTypeDialect::Designer)),
        irc::F_CHOICE_HISTORY_ON_INPUT => fp(&["ChoiceHistoryOnInput"], Codec::EnumText),
        base::F_USE => fp(&["Use"], Codec::EnumText),
        irc::F_INDEXING => fp(&["Indexing"], Codec::EnumText),
        irc::F_FULL_TEXT_SEARCH => fp(&["FullTextSearch"], Codec::EnumText),
        irc::F_DATA_HISTORY => fp(&["DataHistory"], Codec::EnumText),
        _ => return None,
    })
}

pub struct DesignerAttribute;
impl LocusMap for DesignerAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
}
pub struct DesignerTabularSectionAttribute;
impl LocusMap for DesignerTabularSectionAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
}

// ===== TabularSection (recursion-узел) =====
pub struct DesignerTabularSection;
impl LocusMap for DesignerTabularSection {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ts::F_SYNONYM => fp(&["Synonym"], Codec::LocalizedV8),
            ts::F_COMMENT => fp(&["Comment"], Codec::PlainText),
            ts::F_TOOL_TIP => fp(&["ToolTip"], Codec::LocalizedV8),
            ts::F_FILL_CHECKING => fp(&["FillChecking"], Codec::EnumText),
            ts::F_STANDARD_ATTRIBUTES => fp(
                &["StandardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &cat::STD_ATTRS,
                    variant: StdAttrsVariant::Tabular,
                },
            ),
            ts::F_USE => fp(&["Use"], Codec::EnumText),
            ts::F_LINE_NUMBER_LENGTH => fp(&["LineNumberLength"], Codec::IntText),
            _ => return None,
        })
    }
    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            "Attribute" => Some(ChildLocus {
                container: &["ChildObjects"],
                child_tag: "Attribute",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }
    fn child_bindings(&self) -> &'static [ChildBinding] {
        ts_bindings()
    }
}

// ===== Command =====
pub struct DesignerCommand;
impl LocusMap for DesignerCommand {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cmd::F_SYNONYM => fp(&["Synonym"], Codec::LocalizedV8),
            cmd::F_COMMENT => fp(&["Comment"], Codec::PlainText),
            cmd::F_GROUP => fp(&["Group"], Codec::EnumText),
            cmd::F_COMMAND_PARAMETER_TYPE => fp(&["CommandParameterType"], Codec::Type(formats_xml::TypeDialect::Designer)),
            cmd::F_PARAMETER_USE_MODE => fp(&["ParameterUseMode"], Codec::EnumText),
            cmd::F_MODIFIES_DATA => fp(&["ModifiesData"], Codec::BoolText),
            cmd::F_REPRESENTATION => fp(&["Representation"], Codec::EnumText),
            cmd::F_TOOL_TIP => fp(&["ToolTip"], Codec::LocalizedV8),
            cmd::F_PICTURE => fp(&["Picture"], Codec::PictureRef(formats_xml::picture::PictureDialect::Designer)),
            cmd::F_SHORTCUT => fp(&["Shortcut"], Codec::Shortcut),
            cmd::F_ON_MAIN_SERVER_UNAVAILABLE => fp(&["OnMainServerUnavalableBehavior"], Codec::EnumText),
            _ => return None,
        })
    }
}

// FormRef/TemplateRef — bare-ссылки (LocusMap не проецирует свойств).
pub struct DesignerFormRef;
impl LocusMap for DesignerFormRef {
    fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
        None
    }
}
pub struct DesignerTemplateRef;
impl LocusMap for DesignerTemplateRef {
    fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
        None
    }
}

fn ts_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Attribute",
            child_spec: catalog_tabular_section_attribute(),
            child_map: &DesignerTabularSectionAttribute,
        }]
    })
}

fn cat_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: catalog_attribute(), child_map: &DesignerAttribute },
            ChildBinding { collection: "TabularSection", child_spec: catalog_tabular_section(), child_map: &DesignerTabularSection },
            ChildBinding { collection: "Form", child_spec: catalog_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: catalog_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: catalog_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Catalog", spec(), &DesignerCatalog, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Catalog", spec(), &DesignerCatalog, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    catalog()
}

/// Строка R+X-харнесса Designer/Catalog. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Catalog",
    read,
    write,
    corpus_subpath: "coverage/designer/s1_core/Catalogs",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
