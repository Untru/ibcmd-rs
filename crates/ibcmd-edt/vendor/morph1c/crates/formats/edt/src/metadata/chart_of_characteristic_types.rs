//! EDT-проекция вида `ChartOfCharacteristicTypes` + его дочерних видов (зеркало
//! `core/spec/metadata/chart_of_characteristic_types*.rs`, ARCHITECTURE.md §5). ТОЛЬКО
//! размещение/кодировка ячеек — канонику держит `core/spec` (§1.6).
//!
//! Catalog-субстрат на ~90%: РОДИТЕЛЬ — плоские дети корня `.mdo`; дети
//! Attribute/TabularSection/Command — inline (props_wrapped=false); TabularSection —
//! recursion-узел; Forms/Templates — inline-СТАБЫ. Дельта: корневой `type` (root Type) +
//! `characteristicExtValues` (Str-ref) + CCT std-attrs codec. EDT эмитит РАЗРЕЖЁННО;
//! физический порядок свойств задаётся `field_emit_order` (сверено топосортом по корпусу).

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::spec::common::StdAttrsVariant;
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::EntitySpec;

use morph1c_core::spec::catalog_child as base;
use morph1c_core::spec::metadata::chart_of_characteristic_types as cct;
use morph1c_core::spec::metadata::chart_of_characteristic_types::chart_of_characteristic_types;
use morph1c_core::spec::metadata::chart_of_characteristic_types_attribute::chart_of_characteristic_types_attribute;
use morph1c_core::spec::metadata::chart_of_characteristic_types_command as cmd;
use morph1c_core::spec::metadata::chart_of_characteristic_types_command::chart_of_characteristic_types_command;
use morph1c_core::spec::metadata::chart_of_characteristic_types_form_ref as fref;
use morph1c_core::spec::metadata::chart_of_characteristic_types_form_ref::chart_of_characteristic_types_form_ref;
use morph1c_core::spec::metadata::chart_of_characteristic_types_tabular_section as ts;
use morph1c_core::spec::metadata::chart_of_characteristic_types_tabular_section::chart_of_characteristic_types_tabular_section;
use morph1c_core::spec::metadata::chart_of_characteristic_types_tabular_section_attribute::chart_of_characteristic_types_tabular_section_attribute;
use morph1c_core::spec::metadata::chart_of_characteristic_types_template_ref as tref;
use morph1c_core::spec::metadata::chart_of_characteristic_types_template_ref::chart_of_characteristic_types_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ ChartOfCharacteristicTypes =====
pub struct EdtChartOfCharacteristicTypes;

impl LocusMap for EdtChartOfCharacteristicTypes {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cct::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            cct::F_COMMENT => fp(&["comment"], Codec::PlainText),
            cct::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            cct::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            cct::F_CHARACTERISTIC_EXT_VALUES => fp(&["characteristicExtValues"], Codec::PlainText),
            cct::F_TYPE => fp(&["type"], Codec::Type(formats_xml::TypeDialect::Edt)),
            cct::F_HIERARCHICAL => fp(&["hierarchical"], Codec::BoolPresence),
            cct::F_FOLDERS_ON_TOP => fp(&["foldersOnTop"], Codec::BoolPresence),
            cct::F_CODE_LENGTH => fp(&["codeLength"], Codec::IntText),
            cct::F_CODE_ALLOWED_LENGTH => fp(&["codeAllowedLength"], Codec::EnumText),
            cct::F_DESCRIPTION_LENGTH => fp(&["descriptionLength"], Codec::IntText),
            cct::F_CODE_SERIES => fp(&["codeSeries"], Codec::EnumText),
            cct::F_CHECK_UNIQUE => fp(&["checkUnique"], Codec::BoolPresence),
            cct::F_AUTONUMBERING => fp(&["autonumbering"], Codec::BoolPresence),
            cct::F_DEFAULT_PRESENTATION => fp(&["defaultPresentation"], Codec::EnumText),
            cct::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &cct::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            cct::F_CHARACTERISTICS => fp(&["characteristics"], Codec::Characteristics(CharacteristicsDialect::Edt)),
            cct::F_PREDEFINED_DATA_UPDATE => fp(&["predefinedDataUpdate"], Codec::EnumText),
            cct::F_EDIT_TYPE => fp(&["editType"], Codec::EnumText),
            cct::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::BoolPresence),
            cct::F_CHOICE_MODE => fp(&["choiceMode"], Codec::EnumText),
            cct::F_INPUT_BY_STRING => fp(&["inputByString"], Codec::RefList(RefListDialect::Edt)),
            cct::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
            cct::F_SEARCH_STRING_MODE => fp(&["searchStringModeOnInputByString"], Codec::EnumText),
            cct::F_CHOICE_DATA_GET_MODE => fp(&["choiceDataGetModeOnInputByString"], Codec::EnumText),
            cct::F_FTS_ON_INPUT => fp(&["fullTextSearchOnInputByString"], Codec::EnumText),
            cct::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
            cct::F_DEFAULT_OBJECT_FORM => fp(&["defaultObjectForm"], Codec::PlainText),
            cct::F_DEFAULT_FOLDER_FORM => fp(&["defaultFolderForm"], Codec::PlainText),
            cct::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            cct::F_DEFAULT_CHOICE_FORM => fp(&["defaultChoiceForm"], Codec::PlainText),
            cct::F_DEFAULT_FOLDER_CHOICE_FORM => fp(&["defaultFolderChoiceForm"], Codec::PlainText),
            // auxiliary*Form — Designer-only.
            cct::F_BASED_ON => fp(&["basedOn"], Codec::RefList(RefListDialect::Edt)),
            cct::F_DATA_LOCK_FIELDS => fp(&["dataLockFields"], Codec::RefList(RefListDialect::Edt)),
            cct::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            cct::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            cct::F_OBJECT_PRESENTATION => fp(&["objectPresentation"], Codec::LocalizedKeyVal),
            cct::F_EXTENDED_OBJECT_PRESENTATION => fp(&["extendedObjectPresentation"], Codec::LocalizedKeyVal),
            cct::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            cct::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            cct::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            // dataHistory-блок: платформенный EDT НЕСЁТ эти поля (sparse — при не-дефолте).
            cct::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            cct::F_UPDATE_DATA_HISTORY => fp(&["updateDataHistoryImmediatelyAfterWrite"], Codec::BoolPresence),
            cct::F_EXECUTE_AFTER_WRITE_DH => {
                fp(&["executeAfterWriteDataHistoryVersionProcessing"], Codec::BoolPresence)
            }
            cct::F_HELP => fp(&["help"], Codec::HelpConst),
            cct::F_PREDEFINED => fp(&["predefined"], Codec::PredefinedDataCct),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let (tag, name_tag) = match collection {
            "Attribute" => ("attributes", "name"),
            "TabularSection" => ("tabularSections", "name"),
            "Command" => ("commands", "name"),
            "Form" => ("forms", "name"),
            "Template" => ("templates", "name"),
            _ => return None,
        };
        Some(ChildLocus { container: &[], child_tag: tag, props_wrapped: false, name_tag, bare_ref: false })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        cct_bindings()
    }

    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(ROOT_ORDER)
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}

/// EDT физический порядок свойств корня ChartOfCharacteristicTypes (сверено топосортом
/// по корпусу 4/4). `type`/`characteristicExtValues` — после presentations, перед
/// структурным блоком (hierarchical…).
static ROOT_ORDER: &[FieldId] = &[
    cct::F_SYNONYM,
    cct::F_COMMENT,
    cct::F_USE_STANDARD_COMMANDS,
    cct::F_INPUT_BY_STRING,
    cct::F_SEARCH_STRING_MODE,
    cct::F_FTS_ON_INPUT,
    cct::F_CHOICE_DATA_GET_MODE,
    cct::F_STANDARD_ATTRIBUTES,
    cct::F_CHARACTERISTICS,
    cct::F_BASED_ON,
    cct::F_CREATE_ON_INPUT,
    cct::F_INCLUDE_HELP_IN_CONTENTS,
    cct::F_HELP,
    cct::F_DATA_LOCK_FIELDS,
    cct::F_DATA_LOCK_CONTROL_MODE,
    cct::F_FULL_TEXT_SEARCH,
    cct::F_OBJECT_PRESENTATION,
    cct::F_EXTENDED_OBJECT_PRESENTATION,
    cct::F_LIST_PRESENTATION,
    cct::F_EXTENDED_LIST_PRESENTATION,
    cct::F_EXPLANATION,
    cct::F_CHARACTERISTIC_EXT_VALUES,
    // dataHistory — физически ПОСЛЕ presentation-блока, ПЕРЕД type (сверено фикстурой).
    cct::F_DATA_HISTORY,
    cct::F_TYPE,
    cct::F_HIERARCHICAL,
    cct::F_FOLDERS_ON_TOP,
    cct::F_CODE_LENGTH,
    cct::F_CODE_ALLOWED_LENGTH,
    cct::F_DESCRIPTION_LENGTH,
    cct::F_CODE_SERIES,
    cct::F_CHECK_UNIQUE,
    cct::F_AUTONUMBERING,
    cct::F_DEFAULT_PRESENTATION,
    cct::F_PREDEFINED,
    cct::F_PREDEFINED_DATA_UPDATE,
    cct::F_EDIT_TYPE,
    cct::F_QUICK_CHOICE,
    cct::F_CHOICE_MODE,
    cct::F_CHOICE_HISTORY_ON_INPUT,
    cct::F_DEFAULT_OBJECT_FORM,
    cct::F_DEFAULT_FOLDER_FORM,
    cct::F_DEFAULT_LIST_FORM,
    cct::F_DEFAULT_CHOICE_FORM,
    cct::F_DEFAULT_FOLDER_CHOICE_FORM,
    // updateDataHistory/executeAfterWrite — физически В КОНЦЕ корня EDT (сверено фикстурой).
    cct::F_UPDATE_DATA_HISTORY,
    cct::F_EXECUTE_AFTER_WRITE_DH,
];

// ===== Дети Attribute / TabularSection.Attribute (Catalog base) =====
fn base_lookup(field: FieldId) -> Option<FieldProjection> {
    Some(match field {
        irc::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
        irc::F_COMMENT => fp(&["comment"], Codec::PlainText),
        irc::F_TYPE => fp(&["type"], Codec::Type(formats_xml::TypeDialect::Edt)),
        irc::F_PASSWORD_MODE => fp(&["passwordMode"], Codec::BoolPresence),
        irc::F_FORMAT => fp(&["format"], Codec::LocalizedKeyVal),
        irc::F_EDIT_FORMAT => fp(&["editFormat"], Codec::LocalizedKeyVal),
        irc::F_TOOL_TIP => fp(&["toolTip"], Codec::LocalizedKeyVal),
        irc::F_MARK_NEGATIVES => fp(&["markNegatives"], Codec::BoolPresence),
        irc::F_MASK => fp(&["mask"], Codec::PlainText),
        irc::F_MULTI_LINE => fp(&["multiLine"], Codec::BoolPresence),
        irc::F_EXTENDED_EDIT => fp(&["extendedEdit"], Codec::BoolPresence),
        irc::F_MIN_VALUE => fp(&["minValue"], Codec::Value(ValueDialect::Edt)),
        irc::F_MAX_VALUE => fp(&["maxValue"], Codec::Value(ValueDialect::Edt)),
        irc::F_FILL_FROM_FILLING_VALUE => fp(&["fillFromFillingValue"], Codec::BoolPresence),
        irc::F_FILL_VALUE => fp(&["fillValue"], Codec::Value(ValueDialect::Edt)),
        irc::F_FILL_CHECKING => fp(&["fillChecking"], Codec::EnumText),
        irc::F_CHOICE_FOLDERS_AND_ITEMS => fp(&["choiceFoldersAndItems"], Codec::EnumText),
        irc::F_CHOICE_PARAMETER_LINKS => fp(
            &["choiceParameterLinks"],
            Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Edt),
        ),
        irc::F_CHOICE_PARAMETERS => fp(
            &["choiceParameters"],
            Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Edt),
        ),
        irc::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::EnumText),
        irc::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
        // choiceForm — Designer-only.
        irc::F_LINK_BY_TYPE => fp(&["linkByType"], Codec::LinkByType(formats_xml::link_by_type::LinkByTypeDialect::Edt)),
        irc::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
        base::F_USE => fp(&["use"], Codec::EnumText),
        irc::F_INDEXING => fp(&["indexing"], Codec::EnumText),
        irc::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
        irc::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
        _ => return None,
    })
}

/// EDT физический порядок свойств ChartOfCharacteristicTypes.Attribute (root <attributes>).
static ATTRIBUTE_ORDER: &[FieldId] = &[
    irc::F_SYNONYM,
    irc::F_COMMENT,
    irc::F_TYPE,
    irc::F_FORMAT,
    irc::F_EDIT_FORMAT,
    irc::F_TOOL_TIP,
    irc::F_MASK,
    irc::F_MULTI_LINE,
    irc::F_EXTENDED_EDIT,
    irc::F_MIN_VALUE,
    irc::F_MAX_VALUE,
    irc::F_CHOICE_PARAMETER_LINKS,
    irc::F_LINK_BY_TYPE,
    irc::F_CHOICE_PARAMETERS,
    irc::F_FILL_CHECKING,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_FILL_FROM_FILLING_VALUE,
    irc::F_FILL_VALUE,
    irc::F_INDEXING,
    irc::F_FULL_TEXT_SEARCH,
    base::F_USE,
    irc::F_DATA_HISTORY,
];

/// EDT физический порядок свойств TabularSection.Attribute.
static TS_ATTRIBUTE_ORDER: &[FieldId] = &[
    irc::F_SYNONYM,
    irc::F_COMMENT,
    irc::F_TYPE,
    irc::F_FORMAT,
    irc::F_EDIT_FORMAT,
    irc::F_TOOL_TIP,
    irc::F_MIN_VALUE,
    irc::F_MAX_VALUE,
    irc::F_CHOICE_FOLDERS_AND_ITEMS,
    irc::F_FILL_CHECKING,
    irc::F_CHOICE_PARAMETER_LINKS,
    irc::F_CHOICE_PARAMETERS,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_LINK_BY_TYPE,
    irc::F_FILL_FROM_FILLING_VALUE,
    irc::F_FILL_VALUE,
    irc::F_DATA_HISTORY,
    irc::F_INDEXING,
    irc::F_FULL_TEXT_SEARCH,
];

pub struct EdtAttribute;
impl LocusMap for EdtAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(ATTRIBUTE_ORDER)
    }
}

pub struct EdtTabularSectionAttribute;
impl LocusMap for EdtTabularSectionAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(TS_ATTRIBUTE_ORDER)
    }
}

// ===== TabularSection (recursion-узел) =====
pub struct EdtTabularSection;
impl LocusMap for EdtTabularSection {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ts::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            ts::F_COMMENT => fp(&["comment"], Codec::PlainText),
            ts::F_TOOL_TIP => fp(&["toolTip"], Codec::LocalizedKeyVal),
            ts::F_FILL_CHECKING => fp(&["fillChecking"], Codec::EnumText),
            ts::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &cct::STD_ATTRS,
                    variant: StdAttrsVariant::Tabular,
                },
            ),
            ts::F_USE => fp(&["use"], Codec::EnumText),
            // lineNumberLength — ХВОСТОВОЙ лист (физически ПОСЛЕ inline-детей <attributes>,
            // ПЕРЕД <use>; ERP-witnessed недефолты у TS; дефолт 5 EDT омитит).
            ts::F_LINE_NUMBER_LENGTH => fp(&["lineNumberLength"], Codec::IntText),
            _ => return None,
        })
    }
    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            "Attribute" => Some(ChildLocus {
                container: &[],
                child_tag: "attributes",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            }),
            _ => None,
        }
    }
    fn child_bindings(&self) -> &'static [ChildBinding] {
        ts_bindings()
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(TS_ORDER)
    }
    fn trailing_fields(&self) -> &'static [FieldId] {
        // EDT: `<lineNumberLength>` и `<use>` физически ПОСЛЕ inline-детей `<attributes>`
        // (`use` сверен корпусом; lineNumberLength — ERP-witnessed у Document). Взаимный
        // порядок lineNumberLength < use — ПО XCORE-МЕТАМОДЕЛИ
        // (ChartOfCharacteristicTypesTabularSection: [..., attributes, lineNumberLength, use]);
        // co-occurrence в ERP нет (lnl×2, use×11, both×0), корпусом не сверить.
        &[ts::F_LINE_NUMBER_LENGTH, ts::F_USE]
    }
}

/// EDT физический порядок свойств TabularSection (own-fields, сверено топосортом;
/// lineNumberLength < use — по xcore-метамодели, см. trailing_fields).
static TS_ORDER: &[FieldId] = &[
    ts::F_SYNONYM,
    ts::F_TOOL_TIP,
    ts::F_FILL_CHECKING,
    ts::F_STANDARD_ATTRIBUTES,
    ts::F_LINE_NUMBER_LENGTH,
    ts::F_USE,
];

// ===== Command =====
pub struct EdtCommand;
impl LocusMap for EdtCommand {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cmd::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            cmd::F_COMMENT => fp(&["comment"], Codec::PlainText),
            cmd::F_GROUP => fp(&["group"], Codec::EnumText),
            cmd::F_COMMAND_PARAMETER_TYPE => fp(&["commandParameterType"], Codec::Type(formats_xml::TypeDialect::Edt)),
            cmd::F_PARAMETER_USE_MODE => fp(&["parameterUseMode"], Codec::EnumText),
            cmd::F_MODIFIES_DATA => fp(&["modifiesData"], Codec::BoolPresence),
            cmd::F_REPRESENTATION => fp(&["representation"], Codec::EnumText),
            cmd::F_TOOL_TIP => fp(&["toolTip"], Codec::LocalizedKeyVal),
            cmd::F_PICTURE => fp(&["picture"], Codec::PictureRef(formats_xml::picture::PictureDialect::Edt)),
            cmd::F_SHORTCUT => fp(&["shortcut"], Codec::Shortcut),
            // onMainServer* — Designer-only.
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(COMMAND_ORDER)
    }
}

static COMMAND_ORDER: &[FieldId] = &[
    cmd::F_SYNONYM,
    cmd::F_COMMENT,
    cmd::F_GROUP,
    cmd::F_COMMAND_PARAMETER_TYPE,
    cmd::F_PARAMETER_USE_MODE,
    cmd::F_MODIFIES_DATA,
    cmd::F_REPRESENTATION,
    cmd::F_TOOL_TIP,
    cmd::F_PICTURE,
    cmd::F_SHORTCUT,
];

// ===== FormRef / TemplateRef (EDT — полный стаб) =====
pub struct EdtFormRef;
impl LocusMap for EdtFormRef {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            fref::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            fref::F_COMMENT => fp(&["comment"], Codec::PlainText),
            fref::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            fref::F_HELP => fp(&["help"], Codec::HelpConst),
            fref::F_USE_PURPOSES => fp(&["usePurposes"], Codec::UsePurposesConst),
            _ => return None,
        })
    }
}

pub struct EdtTemplateRef;
impl LocusMap for EdtTemplateRef {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            tref::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            tref::F_COMMENT => fp(&["comment"], Codec::PlainText),
            tref::F_TEMPLATE_TYPE => fp(&["templateType"], Codec::EnumText),
            _ => return None,
        })
    }
}

/// `&'static` бинды child-видов TabularSection (вложенный Attribute).
fn ts_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Attribute",
            child_spec: chart_of_characteristic_types_tabular_section_attribute(),
            child_map: &EdtTabularSectionAttribute,
        }]
    })
}

/// `&'static` бинды child-видов корня ChartOfCharacteristicTypes.
fn cct_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: chart_of_characteristic_types_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "TabularSection", child_spec: chart_of_characteristic_types_tabular_section(), child_map: &EdtTabularSection },
            ChildBinding { collection: "Form", child_spec: chart_of_characteristic_types_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: chart_of_characteristic_types_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: chart_of_characteristic_types_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ChartOfCharacteristicTypes", spec(), &EdtChartOfCharacteristicTypes, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ChartOfCharacteristicTypes", spec(), &EdtChartOfCharacteristicTypes, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    chart_of_characteristic_types()
}

/// Строка R+X-харнесса EDT/ChartOfCharacteristicTypes. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "ChartOfCharacteristicTypes",
    read,
    write,
    corpus_subpath: "coverage/edt/s2_registers/src/ChartsOfCharacteristicTypes",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
