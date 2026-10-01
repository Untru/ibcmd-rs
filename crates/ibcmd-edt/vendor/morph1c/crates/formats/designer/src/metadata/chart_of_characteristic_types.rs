//! Designer-проекция вида `ChartOfCharacteristicTypes` + его дочерних видов (зеркало
//! `core/spec/metadata/chart_of_characteristic_types*.rs`, ARCHITECTURE.md §5; child-objects
//! substrate). ТОЛЬКО размещение/кодировка — канонику держит `core/spec` (§1.6). Иной
//! синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5), включая детей.
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<ChartOfCharacteristicTypes>/<Properties>`,
//! DENSE. Дети Attribute/TabularSection/Command — `<ChildObjects>`+`<Properties>`
//! (props_wrapped=true); TabularSection — recursion-узел; Forms/Templates — BARE-ссылки
//! `<Form>Имя</Form>`. Дельта vs Catalog: корневой `Type` (root Type) +
//! `CharacteristicExtValues` (Str-ref) + CCT std-attrs codec.

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::catalog_child as base;
use morph1c_core::spec::metadata::chart_of_characteristic_types as cct;
use morph1c_core::spec::metadata::chart_of_characteristic_types::chart_of_characteristic_types;
use morph1c_core::spec::metadata::chart_of_characteristic_types_attribute::chart_of_characteristic_types_attribute;
use morph1c_core::spec::metadata::chart_of_characteristic_types_command as cmd;
use morph1c_core::spec::metadata::chart_of_characteristic_types_command::chart_of_characteristic_types_command;
use morph1c_core::spec::metadata::chart_of_characteristic_types_form_ref::chart_of_characteristic_types_form_ref;
use morph1c_core::spec::metadata::chart_of_characteristic_types_tabular_section as ts;
use morph1c_core::spec::metadata::chart_of_characteristic_types_tabular_section::chart_of_characteristic_types_tabular_section;
use morph1c_core::spec::metadata::chart_of_characteristic_types_tabular_section_attribute::chart_of_characteristic_types_tabular_section_attribute;
use morph1c_core::spec::metadata::chart_of_characteristic_types_template_ref::chart_of_characteristic_types_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

// Пути от корня <MetaDataObject> через <ChartOfCharacteristicTypes>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["ChartOfCharacteristicTypes", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerChartOfCharacteristicTypes;

impl LocusMap for DesignerChartOfCharacteristicTypes {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cct::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            cct::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            cct::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            cct::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            cct::F_CHARACTERISTIC_EXT_VALUES => fp(p!("CharacteristicExtValues"), Codec::PlainText),
            cct::F_TYPE => fp(p!("Type"), Codec::Type(formats_xml::TypeDialect::Designer)),
            cct::F_HIERARCHICAL => fp(p!("Hierarchical"), Codec::BoolText),
            cct::F_FOLDERS_ON_TOP => fp(p!("FoldersOnTop"), Codec::BoolText),
            cct::F_CODE_LENGTH => fp(p!("CodeLength"), Codec::IntText),
            cct::F_CODE_ALLOWED_LENGTH => fp(p!("CodeAllowedLength"), Codec::EnumText),
            cct::F_DESCRIPTION_LENGTH => fp(p!("DescriptionLength"), Codec::IntText),
            cct::F_CODE_SERIES => fp(p!("CodeSeries"), Codec::EnumText),
            cct::F_CHECK_UNIQUE => fp(p!("CheckUnique"), Codec::BoolText),
            cct::F_AUTONUMBERING => fp(p!("Autonumbering"), Codec::BoolText),
            cct::F_DEFAULT_PRESENTATION => fp(p!("DefaultPresentation"), Codec::EnumText),
            cct::F_STANDARD_ATTRIBUTES => fp(
                p!("StandardAttributes"),
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &cct::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            cct::F_CHARACTERISTICS => fp(p!("Characteristics"), Codec::Characteristics(CharacteristicsDialect::Designer)),
            cct::F_PREDEFINED_DATA_UPDATE => fp(p!("PredefinedDataUpdate"), Codec::EnumText),
            cct::F_EDIT_TYPE => fp(p!("EditType"), Codec::EnumText),
            cct::F_QUICK_CHOICE => fp(p!("QuickChoice"), Codec::BoolText),
            cct::F_CHOICE_MODE => fp(p!("ChoiceMode"), Codec::EnumText),
            cct::F_INPUT_BY_STRING => fp(p!("InputByString"), Codec::RefList(RefListDialect::DesignerField)),
            cct::F_CREATE_ON_INPUT => fp(p!("CreateOnInput"), Codec::EnumText),
            cct::F_SEARCH_STRING_MODE => fp(p!("SearchStringModeOnInputByString"), Codec::EnumText),
            cct::F_CHOICE_DATA_GET_MODE => fp(p!("ChoiceDataGetModeOnInputByString"), Codec::EnumText),
            cct::F_FTS_ON_INPUT => fp(p!("FullTextSearchOnInputByString"), Codec::EnumText),
            cct::F_CHOICE_HISTORY_ON_INPUT => fp(p!("ChoiceHistoryOnInput"), Codec::EnumText),
            cct::F_DEFAULT_OBJECT_FORM => fp(p!("DefaultObjectForm"), Codec::PlainText),
            cct::F_DEFAULT_FOLDER_FORM => fp(p!("DefaultFolderForm"), Codec::PlainText),
            cct::F_DEFAULT_LIST_FORM => fp(p!("DefaultListForm"), Codec::PlainText),
            cct::F_DEFAULT_CHOICE_FORM => fp(p!("DefaultChoiceForm"), Codec::PlainText),
            cct::F_DEFAULT_FOLDER_CHOICE_FORM => fp(p!("DefaultFolderChoiceForm"), Codec::PlainText),
            cct::F_AUX_OBJECT_FORM => fp(p!("AuxiliaryObjectForm"), Codec::PlainText),
            cct::F_AUX_FOLDER_FORM => fp(p!("AuxiliaryFolderForm"), Codec::PlainText),
            cct::F_AUX_LIST_FORM => fp(p!("AuxiliaryListForm"), Codec::PlainText),
            cct::F_AUX_CHOICE_FORM => fp(p!("AuxiliaryChoiceForm"), Codec::PlainText),
            cct::F_AUX_FOLDER_CHOICE_FORM => fp(p!("AuxiliaryFolderChoiceForm"), Codec::PlainText),
            cct::F_BASED_ON => fp(p!("BasedOn"), Codec::RefList(RefListDialect::DesignerItem)),
            cct::F_DATA_LOCK_FIELDS => fp(p!("DataLockFields"), Codec::RefList(RefListDialect::DesignerField)),
            cct::F_DATA_LOCK_CONTROL_MODE => fp(p!("DataLockControlMode"), Codec::EnumText),
            cct::F_FULL_TEXT_SEARCH => fp(p!("FullTextSearch"), Codec::EnumText),
            cct::F_OBJECT_PRESENTATION => fp(p!("ObjectPresentation"), Codec::LocalizedV8),
            cct::F_EXTENDED_OBJECT_PRESENTATION => fp(p!("ExtendedObjectPresentation"), Codec::LocalizedV8),
            cct::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            cct::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            cct::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            cct::F_DATA_HISTORY => fp(p!("DataHistory"), Codec::EnumText),
            cct::F_UPDATE_DATA_HISTORY => fp(p!("UpdateDataHistoryImmediatelyAfterWrite"), Codec::BoolText),
            cct::F_EXECUTE_AFTER_WRITE_DH => fp(p!("ExecuteAfterWriteDataHistoryVersionProcessing"), Codec::BoolText),
            // help/predefined — EDT-only (Designer не несёт): not projected.
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["ChartOfCharacteristicTypes", "ChildObjects"];
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
        cct_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // Все Designer-ChartOfCharacteristicTypes несут `<ChildObjects>` (пустой при нуле детей).
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

/// Designer DENSE-порядок свойств `ChartOfCharacteristicTypes.Attribute`. Отличается от
/// `Catalog.Attribute` РОВНО перестановкой `Indexing`↔`Use`: у плана видов характеристик
/// `<Indexing>` идёт ПЕРЕД `<Use>` (сверено корпусом), тогда как спек-порядок (общий с
/// Catalog) — `Use` перед `Indexing`. Прочее = спек-порядок.
static DESIGNER_ATTRIBUTE_ORDER: &[FieldId] = &[
    irc::F_SYNONYM,
    irc::F_COMMENT,
    irc::F_TYPE,
    irc::F_PASSWORD_MODE,
    irc::F_FORMAT,
    irc::F_EDIT_FORMAT,
    irc::F_TOOL_TIP,
    irc::F_MARK_NEGATIVES,
    irc::F_MASK,
    irc::F_MULTI_LINE,
    irc::F_EXTENDED_EDIT,
    irc::F_MIN_VALUE,
    irc::F_MAX_VALUE,
    irc::F_FILL_FROM_FILLING_VALUE,
    irc::F_FILL_VALUE,
    irc::F_FILL_CHECKING,
    irc::F_CHOICE_FOLDERS_AND_ITEMS,
    irc::F_CHOICE_PARAMETER_LINKS,
    irc::F_CHOICE_PARAMETERS,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_CHOICE_FORM,
    irc::F_LINK_BY_TYPE,
    irc::F_CHOICE_HISTORY_ON_INPUT,
    irc::F_INDEXING,
    base::F_USE,
    irc::F_FULL_TEXT_SEARCH,
    irc::F_DATA_HISTORY,
];

pub struct DesignerAttribute;
impl LocusMap for DesignerAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(DESIGNER_ATTRIBUTE_ORDER)
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
                    decl: &cct::STD_ATTRS,
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
            child_spec: chart_of_characteristic_types_tabular_section_attribute(),
            child_map: &DesignerTabularSectionAttribute,
        }]
    })
}

fn cct_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: chart_of_characteristic_types_attribute(), child_map: &DesignerAttribute },
            ChildBinding { collection: "TabularSection", child_spec: chart_of_characteristic_types_tabular_section(), child_map: &DesignerTabularSection },
            ChildBinding { collection: "Form", child_spec: chart_of_characteristic_types_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: chart_of_characteristic_types_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: chart_of_characteristic_types_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ChartOfCharacteristicTypes", spec(), &DesignerChartOfCharacteristicTypes, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ChartOfCharacteristicTypes", spec(), &DesignerChartOfCharacteristicTypes, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    chart_of_characteristic_types()
}

/// Строка R+X-харнесса Designer/ChartOfCharacteristicTypes. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "ChartOfCharacteristicTypes",
    read,
    write,
    corpus_subpath: "coverage/designer/s2_registers/ChartsOfCharacteristicTypes",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
