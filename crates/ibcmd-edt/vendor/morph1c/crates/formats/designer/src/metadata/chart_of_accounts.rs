//! Designer-проекция вида `ChartOfAccounts` + дочерних видов (зеркало
//! `core/spec/metadata/chart_of_accounts*.rs`, ARCHITECTURE.md §5). Ссылочно-порождающий вид,
//! ближайший к `ChartOfCharacteristicTypes`, но с бухгалтерской спецификой (basedOn/
//! extDimensionTypes/maxExtDimensionCount/codeMask/autoOrderByCode/orderLength/codeSeries +
//! СВОЙ std-attrs-блок `coa::STD_ATTRS` — 10 атрибутов, ERP-witnessed), БЕЗ иерархии (нет
//! hierarchical/foldersOnTop/codeAllowedLength/autonumbering/type/defaultFolderForm). Дети
//! Attribute/TabularSection/AccountingFlag/ExtDimensionAccountingFlag/Command —
//! `<ChildObjects>`; Form/Template — BARE-ссылки. Порядок = спек-DENSE (сверено фикстурой +
//! ERP designer_8.3.27). Признаки учёта — attribute-подобные дети БЕЗ Indexing/FullTextSearch
//! (26 DENSE-листьев, ERP-witnessed).
//!
//! `StandardTabularSections` — `Codec::StdTabularSections` (декларация
//! `coa::STD_TABULAR_SECTIONS`; ERP-witnessed). predefined Designer несёт сайдкаром
//! `Ext/Predefined.xml` (`xsi:type="ChartOfAccountsPredefinedItems"` — грамматика ≠
//! Catalog, кодека нет, см. отчёт).

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::chart_of_accounts as coa;
use morph1c_core::spec::metadata::chart_of_accounts::chart_of_accounts;
use morph1c_core::spec::metadata::chart_of_accounts_accounting_flag::chart_of_accounts_accounting_flag;
use morph1c_core::spec::metadata::chart_of_accounts_attribute::chart_of_accounts_attribute;
use morph1c_core::spec::metadata::chart_of_accounts_ext_dimension_accounting_flag::chart_of_accounts_ext_dimension_accounting_flag;
use morph1c_core::spec::metadata::chart_of_accounts_command as cmd;
use morph1c_core::spec::metadata::chart_of_accounts_command::chart_of_accounts_command;
use morph1c_core::spec::metadata::chart_of_accounts_form_ref::chart_of_accounts_form_ref;
use morph1c_core::spec::metadata::chart_of_accounts_tabular_section as ts;
use morph1c_core::spec::metadata::chart_of_accounts_tabular_section::chart_of_accounts_tabular_section;
use morph1c_core::spec::metadata::chart_of_accounts_tabular_section_attribute::chart_of_accounts_tabular_section_attribute;
use morph1c_core::spec::metadata::chart_of_accounts_template_ref::chart_of_accounts_template_ref;
// TabularSection std-attrs codec переиспользует CCT-декларацию (tabular-часть кинд-агностична,
// coverage планов счетов ТЧ не несёт → путь не витнессится).
use morph1c_core::spec::metadata::chart_of_characteristic_types as cct;
use morph1c_core::spec::ir_child as irc;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

macro_rules! p {
    ($tag:literal) => {
        &["ChartOfAccounts", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerChartOfAccounts;

impl LocusMap for DesignerChartOfAccounts {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            coa::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            coa::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            coa::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            coa::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            coa::F_BASED_ON => fp(p!("BasedOn"), Codec::RefList(RefListDialect::DesignerItem)),
            coa::F_EXT_DIMENSION_TYPES => fp(p!("ExtDimensionTypes"), Codec::PlainText),
            coa::F_MAX_EXT_DIMENSION_COUNT => fp(p!("MaxExtDimensionCount"), Codec::IntText),
            coa::F_CODE_MASK => fp(p!("CodeMask"), Codec::PlainText),
            coa::F_CODE_LENGTH => fp(p!("CodeLength"), Codec::IntText),
            coa::F_DESCRIPTION_LENGTH => fp(p!("DescriptionLength"), Codec::IntText),
            coa::F_CODE_SERIES => fp(p!("CodeSeries"), Codec::EnumText),
            coa::F_CHECK_UNIQUE => fp(p!("CheckUnique"), Codec::BoolText),
            coa::F_DEFAULT_PRESENTATION => fp(p!("DefaultPresentation"), Codec::EnumText),
            coa::F_STANDARD_ATTRIBUTES => fp(
                p!("StandardAttributes"),
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &coa::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            coa::F_CHARACTERISTICS => fp(p!("Characteristics"), Codec::Characteristics(CharacteristicsDialect::Designer)),
            // Стандартная ТЧ ExtDimensionTypes с вложенным <xr:StandardAttributes>
            // (ERP-witnessed оба объекта; кодек навигирует от корня сам — локус
            // документирует позицию DENSE между Characteristics и PredefinedDataUpdate).
            coa::F_STANDARD_TABULAR_SECTIONS => fp(
                p!("StandardTabularSections"),
                Codec::StdTabularSections {
                    dialect: GenDialect::Designer,
                    decl: &coa::STD_TABULAR_SECTIONS,
                },
            ),
            coa::F_PREDEFINED_DATA_UPDATE => fp(p!("PredefinedDataUpdate"), Codec::EnumText),
            coa::F_EDIT_TYPE => fp(p!("EditType"), Codec::EnumText),
            coa::F_QUICK_CHOICE => fp(p!("QuickChoice"), Codec::BoolText),
            coa::F_CHOICE_MODE => fp(p!("ChoiceMode"), Codec::EnumText),
            coa::F_INPUT_BY_STRING => fp(p!("InputByString"), Codec::RefList(RefListDialect::DesignerField)),
            coa::F_SEARCH_STRING_MODE => fp(p!("SearchStringModeOnInputByString"), Codec::EnumText),
            coa::F_FTS_ON_INPUT => fp(p!("FullTextSearchOnInputByString"), Codec::EnumText),
            coa::F_CHOICE_DATA_GET_MODE => fp(p!("ChoiceDataGetModeOnInputByString"), Codec::EnumText),
            coa::F_CREATE_ON_INPUT => fp(p!("CreateOnInput"), Codec::EnumText),
            coa::F_CHOICE_HISTORY_ON_INPUT => fp(p!("ChoiceHistoryOnInput"), Codec::EnumText),
            coa::F_DEFAULT_OBJECT_FORM => fp(p!("DefaultObjectForm"), Codec::PlainText),
            coa::F_DEFAULT_LIST_FORM => fp(p!("DefaultListForm"), Codec::PlainText),
            coa::F_DEFAULT_CHOICE_FORM => fp(p!("DefaultChoiceForm"), Codec::PlainText),
            coa::F_AUX_OBJECT_FORM => fp(p!("AuxiliaryObjectForm"), Codec::PlainText),
            coa::F_AUX_LIST_FORM => fp(p!("AuxiliaryListForm"), Codec::PlainText),
            coa::F_AUX_CHOICE_FORM => fp(p!("AuxiliaryChoiceForm"), Codec::PlainText),
            coa::F_AUTO_ORDER_BY_CODE => fp(p!("AutoOrderByCode"), Codec::BoolText),
            coa::F_ORDER_LENGTH => fp(p!("OrderLength"), Codec::IntText),
            coa::F_DATA_LOCK_FIELDS => fp(p!("DataLockFields"), Codec::RefList(RefListDialect::DesignerField)),
            coa::F_DATA_LOCK_CONTROL_MODE => fp(p!("DataLockControlMode"), Codec::EnumText),
            coa::F_FULL_TEXT_SEARCH => fp(p!("FullTextSearch"), Codec::EnumText),
            coa::F_DATA_HISTORY => fp(p!("DataHistory"), Codec::EnumText),
            coa::F_UPDATE_DATA_HISTORY => fp(p!("UpdateDataHistoryImmediatelyAfterWrite"), Codec::BoolText),
            coa::F_EXECUTE_AFTER_WRITE_DH => fp(p!("ExecuteAfterWriteDataHistoryVersionProcessing"), Codec::BoolText),
            coa::F_OBJECT_PRESENTATION => fp(p!("ObjectPresentation"), Codec::LocalizedV8),
            coa::F_EXTENDED_OBJECT_PRESENTATION => fp(p!("ExtendedObjectPresentation"), Codec::LocalizedV8),
            coa::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            coa::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            coa::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            // help/predefined — EDT-only (Designer не несёт).
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["ChartOfAccounts", "ChildObjects"];
        let (tag, bare) = match collection {
            "Attribute" => ("Attribute", false),
            "TabularSection" => ("TabularSection", false),
            "AccountingFlag" => ("AccountingFlag", false),
            "ExtDimensionAccountingFlag" => ("ExtDimensionAccountingFlag", false),
            "Command" => ("Command", false),
            "Form" => ("Form", true),
            "Template" => ("Template", true),
            _ => return None,
        };
        Some(ChildLocus { container: cont, child_tag: tag, props_wrapped: !bare, name_tag: "Name", bare_ref: bare })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        coa_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        true
    }
}

// ===== Дети Attribute / TabularSection.Attribute (IR base, БЕЗ use) =====
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

/// Признак учёта / признак учёта субконто — attribute-подобный ребёнок; DENSE-порядок и
/// набор листьев задаёт СПЕК (без Indexing/FullTextSearch — их нет в spec-полях вида,
/// ERP-witnessed 26 листьев), lookup переиспользует базовую таблицу.
pub struct DesignerAccountingFlag;
impl LocusMap for DesignerAccountingFlag {
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
            child_spec: chart_of_accounts_tabular_section_attribute(),
            child_map: &DesignerTabularSectionAttribute,
        }]
    })
}

fn coa_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: chart_of_accounts_attribute(), child_map: &DesignerAttribute },
            ChildBinding { collection: "TabularSection", child_spec: chart_of_accounts_tabular_section(), child_map: &DesignerTabularSection },
            ChildBinding { collection: "AccountingFlag", child_spec: chart_of_accounts_accounting_flag(), child_map: &DesignerAccountingFlag },
            ChildBinding { collection: "ExtDimensionAccountingFlag", child_spec: chart_of_accounts_ext_dimension_accounting_flag(), child_map: &DesignerAccountingFlag },
            ChildBinding { collection: "Form", child_spec: chart_of_accounts_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: chart_of_accounts_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: chart_of_accounts_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ChartOfAccounts", spec(), &DesignerChartOfAccounts, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ChartOfAccounts", spec(), &DesignerChartOfAccounts, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    chart_of_accounts()
}

/// Строка R+X-харнесса Designer/ChartOfAccounts. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "ChartOfAccounts",
    read,
    write,
    corpus_subpath: "coverage/designer/s2_registers/ChartsOfAccounts",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
