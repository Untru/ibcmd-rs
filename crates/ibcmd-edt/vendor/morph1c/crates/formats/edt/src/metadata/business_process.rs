//! EDT-проекция вида `BusinessProcess` + его дочерних видов (зеркало
//! `core/spec/metadata/business_process*.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Переиспользует Document/Task-
//! субстрат (`ir_child` base-реквизит, ref-list/characteristics/value/type/cpl/picture/
//! help-кодеки, children-рекурсию) + BusinessProcess-дельту (std_attrs_business_process,
//! номерные поля, `task`/`createTaskInPrivilegedMode`).
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дети Attribute/TabularSection/Command — inline
//! (props_wrapped=false); TabularSection — recursion-узел. Forms/Templates — inline-СТАБЫ.
//! EDT эмитит РАЗРЕЖЁННО; физический порядок свойств — `field_emit_order`. Графическая
//! карта маршрута (Flowchart.scheme) — отдельный файл, НЕ дескриптор → не проецируется.

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::business_process as bp;
use morph1c_core::spec::metadata::business_process::business_process;
use morph1c_core::spec::metadata::business_process_attribute::business_process_attribute;
use morph1c_core::spec::metadata::business_process_command as cmd;
use morph1c_core::spec::metadata::business_process_command::business_process_command;
use morph1c_core::spec::metadata::business_process_form_ref as fref;
use morph1c_core::spec::metadata::business_process_form_ref::business_process_form_ref;
use morph1c_core::spec::metadata::business_process_tabular_section as ts;
use morph1c_core::spec::metadata::business_process_tabular_section::business_process_tabular_section;
use morph1c_core::spec::metadata::business_process_tabular_section_attribute::business_process_tabular_section_attribute;
use morph1c_core::spec::metadata::business_process_template_ref as tref;
use morph1c_core::spec::metadata::business_process_template_ref::business_process_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ BusinessProcess =====
pub struct EdtBusinessProcess;

impl LocusMap for EdtBusinessProcess {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            bp::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            bp::F_COMMENT => fp(&["comment"], Codec::PlainText),
            bp::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            bp::F_EDIT_TYPE => fp(&["editType"], Codec::EnumText),
            bp::F_INPUT_BY_STRING => fp(&["inputByString"], Codec::RefList(RefListDialect::Edt)),
            bp::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
            bp::F_SEARCH_STRING_MODE => fp(&["searchStringModeOnInputByString"], Codec::EnumText),
            bp::F_CHOICE_DATA_GET_MODE => fp(&["choiceDataGetModeOnInputByString"], Codec::EnumText),
            bp::F_FTS_ON_INPUT => fp(&["fullTextSearchOnInputByString"], Codec::EnumText),
            bp::F_DEFAULT_OBJECT_FORM => fp(&["defaultObjectForm"], Codec::PlainText),
            bp::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            bp::F_DEFAULT_CHOICE_FORM => fp(&["defaultChoiceForm"], Codec::PlainText),
            // auxiliary*Form — Designer-only.
            bp::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
            bp::F_NUMBER_TYPE => fp(&["numberType"], Codec::EnumText),
            bp::F_NUMBER_LENGTH => fp(&["numberLength"], Codec::IntText),
            bp::F_NUMBER_ALLOWED_LENGTH => fp(&["numberAllowedLength"], Codec::EnumText),
            bp::F_CHECK_UNIQUE => fp(&["checkUnique"], Codec::BoolPresence),
            bp::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &bp::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            bp::F_CHARACTERISTICS => fp(&["characteristics"], Codec::Characteristics(CharacteristicsDialect::Edt)),
            bp::F_AUTONUMBERING => fp(&["autonumbering"], Codec::BoolPresence),
            bp::F_BASED_ON => fp(&["basedOn"], Codec::RefList(RefListDialect::Edt)),
            bp::F_NUMBER_PERIODICITY => fp(&["numberPeriodicity"], Codec::EnumText),
            bp::F_TASK => fp(&["task"], Codec::PlainText),
            bp::F_CREATE_TASK_IN_PRIVILEGED_MODE => fp(&["createTaskInPrivilegedMode"], Codec::BoolPresence),
            bp::F_DATA_LOCK_FIELDS => fp(&["dataLockFields"], Codec::RefList(RefListDialect::Edt)),
            bp::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            bp::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            bp::F_HELP => fp(&["help"], Codec::HelpConst),
            bp::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            bp::F_OBJECT_PRESENTATION => fp(&["objectPresentation"], Codec::LocalizedKeyVal),
            bp::F_EXTENDED_OBJECT_PRESENTATION => fp(&["extendedObjectPresentation"], Codec::LocalizedKeyVal),
            bp::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            bp::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            bp::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            // dataHistory-блок: платформенный EDT НЕСЁТ эти поля (sparse — при не-дефолте).
            bp::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            bp::F_UPDATE_DATA_HISTORY => fp(&["updateDataHistoryImmediatelyAfterWrite"], Codec::BoolPresence),
            bp::F_EXECUTE_AFTER_WRITE_DH => {
                fp(&["executeAfterWriteDataHistoryVersionProcessing"], Codec::BoolPresence)
            }
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
        business_process_bindings()
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

/// EDT физический порядок свойств корня BusinessProcess (сверено по корпусу).
static ROOT_ORDER: &[FieldId] = &[
    bp::F_SYNONYM,
    bp::F_COMMENT,
    bp::F_USE_STANDARD_COMMANDS,
    bp::F_INPUT_BY_STRING,
    bp::F_SEARCH_STRING_MODE,
    bp::F_FTS_ON_INPUT,
    bp::F_CHOICE_DATA_GET_MODE,
    bp::F_STANDARD_ATTRIBUTES,
    bp::F_CHARACTERISTICS,
    bp::F_BASED_ON,
    bp::F_CREATE_ON_INPUT,
    bp::F_INCLUDE_HELP_IN_CONTENTS,
    bp::F_HELP,
    bp::F_DATA_LOCK_FIELDS,
    bp::F_DATA_LOCK_CONTROL_MODE,
    bp::F_FULL_TEXT_SEARCH,
    bp::F_OBJECT_PRESENTATION,
    bp::F_LIST_PRESENTATION,
    bp::F_EXTENDED_LIST_PRESENTATION,
    bp::F_EXPLANATION,
    // dataHistory — физически ПОСЛЕ presentation-блока, ПЕРЕД editType (сверено фикстурой).
    bp::F_DATA_HISTORY,
    bp::F_EDIT_TYPE,
    bp::F_DEFAULT_OBJECT_FORM,
    bp::F_DEFAULT_LIST_FORM,
    bp::F_DEFAULT_CHOICE_FORM,
    // choiceHistoryOnInput — МЕЖДУ default*Form и numberType (ERP-witness 6/19 носителей,
    // structural-tail T1: `…defaultListForm choiceHistoryOnInput numberType…`;
    // SSL-объект поля не несёт — прежняя хвостовая позиция была невитнессированной).
    bp::F_CHOICE_HISTORY_ON_INPUT,
    bp::F_NUMBER_TYPE,
    bp::F_NUMBER_LENGTH,
    bp::F_NUMBER_ALLOWED_LENGTH,
    bp::F_CHECK_UNIQUE,
    bp::F_AUTONUMBERING,
    bp::F_NUMBER_PERIODICITY,
    bp::F_TASK,
    bp::F_CREATE_TASK_IN_PRIVILEGED_MODE,
    // updateDataHistory/executeAfterWrite — физически В КОНЦЕ корня EDT (сверено фикстурой).
    bp::F_UPDATE_DATA_HISTORY,
    bp::F_EXECUTE_AFTER_WRITE_DH,
];

// ===== Дети Attribute / TabularSection.Attribute (ir_child base) =====
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
        // choiceParameters/choiceForm/linkByType — Designer-only (EDT не несёт).
        irc::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::EnumText),
        irc::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
        irc::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
        irc::F_INDEXING => fp(&["indexing"], Codec::EnumText),
        irc::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
        irc::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
        _ => return None,
    })
}

/// EDT физический порядок свойств BusinessProcess.Attribute (root <attributes>).
static ATTRIBUTE_ORDER: &[FieldId] = &[
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
    irc::F_CHOICE_FOLDERS_AND_ITEMS,
    irc::F_CHOICE_PARAMETER_LINKS,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_CHOICE_HISTORY_ON_INPUT,
    irc::F_FILL_CHECKING,
    irc::F_FILL_FROM_FILLING_VALUE,
    irc::F_FILL_VALUE,
    irc::F_INDEXING,
    irc::F_FULL_TEXT_SEARCH,
    irc::F_DATA_HISTORY,
];

/// EDT физический порядок свойств BusinessProcess.TabularSection.Attribute. В TS-реквизите
/// `choiceParameterLinks` идёт ПОСЛЕ `fillChecking` (сверено по ERP: реквизит «Отпуск» ТЧ
/// «Отпуска» — `…maxValue fillChecking choiceParameterLinks dataHistory…`; так же
/// Отсутствие/Справка2НДФЛ). Прежняя позиция (перед choiceFoldersAndItems) была
/// невитнессирована — в SSL-«Задании» ТЧ нет.
static TS_ATTRIBUTE_ORDER: &[FieldId] = &[
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
    irc::F_CHOICE_FOLDERS_AND_ITEMS,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_CHOICE_HISTORY_ON_INPUT,
    irc::F_FILL_CHECKING,
    irc::F_CHOICE_PARAMETER_LINKS,
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
                    decl: &bp::STD_ATTRS,
                    variant: StdAttrsVariant::Tabular,
                },
            ),
            // lineNumberLength — ХВОСТОВОЙ лист (физически ПОСЛЕ inline-детей <attributes>;
            // ERP-witnessed недефолты у TS; дефолт 5 EDT омитит).
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
        // EDT: <lineNumberLength> физически ПОСЛЕ inline-детей <attributes> (как у Document,
        // ERP-witnessed).
        &[ts::F_LINE_NUMBER_LENGTH]
    }
}

/// EDT физический порядок свойств TabularSection (own-fields).
static TS_ORDER: &[FieldId] = &[
    ts::F_SYNONYM,
    ts::F_COMMENT,
    ts::F_TOOL_TIP,
    ts::F_FILL_CHECKING,
    ts::F_STANDARD_ATTRIBUTES,
    ts::F_LINE_NUMBER_LENGTH,
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
            fref::F_FORM_TYPE => fp(&["formType"], Codec::EnumText),
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
            child_spec: business_process_tabular_section_attribute(),
            child_map: &EdtTabularSectionAttribute,
        }]
    })
}

/// `&'static` бинды child-видов корня BusinessProcess.
fn business_process_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: business_process_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "TabularSection", child_spec: business_process_tabular_section(), child_map: &EdtTabularSection },
            ChildBinding { collection: "Form", child_spec: business_process_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: business_process_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: business_process_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("BusinessProcess", spec(), &EdtBusinessProcess, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("BusinessProcess", spec(), &EdtBusinessProcess, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    business_process()
}

/// Строка R+X-харнесса EDT/BusinessProcess. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "BusinessProcess",
    read,
    write,
    corpus_subpath: "coverage/edt/s3_bizproc/src/BusinessProcesses",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
