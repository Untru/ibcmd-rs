//! EDT-проекция вида `Task` + его дочерних видов (зеркало `core/spec/metadata/task*.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Переиспользует Catalog/Document-субстрат (`ir_child` base-реквизит, ref-list/
//! characteristics/value/type/cpl/picture/help-кодеки, children-рекурсию) + Task-дельту
//! (std_attrs_task, номерные/адресные поля, НОВАЯ коллекция AddressingAttribute).
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дети Attribute/AddressingAttribute/TabularSection/
//! Command — inline (props_wrapped=false); TabularSection — recursion-узел. Forms/Templates
//! — inline-СТАБЫ. EDT эмитит РАЗРЕЖЁННО; физический порядок свойств — `field_emit_order`.

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::task as tsk;
use morph1c_core::spec::metadata::task::task;
use morph1c_core::spec::metadata::task_addressing_attribute as aattr;
use morph1c_core::spec::metadata::task_addressing_attribute::task_addressing_attribute;
use morph1c_core::spec::metadata::task_attribute::task_attribute;
use morph1c_core::spec::metadata::task_command as cmd;
use morph1c_core::spec::metadata::task_command::task_command;
use morph1c_core::spec::metadata::task_form_ref as fref;
use morph1c_core::spec::metadata::task_form_ref::task_form_ref;
use morph1c_core::spec::metadata::task_tabular_section as ts;
use morph1c_core::spec::metadata::task_tabular_section::task_tabular_section;
use morph1c_core::spec::metadata::task_tabular_section_attribute::task_tabular_section_attribute;
use morph1c_core::spec::metadata::task_template_ref as tref;
use morph1c_core::spec::metadata::task_template_ref::task_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ Task =====
pub struct EdtTask;

impl LocusMap for EdtTask {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            tsk::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            tsk::F_COMMENT => fp(&["comment"], Codec::PlainText),
            tsk::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            tsk::F_NUMBER_TYPE => fp(&["numberType"], Codec::EnumText),
            tsk::F_NUMBER_LENGTH => fp(&["numberLength"], Codec::IntText),
            tsk::F_NUMBER_ALLOWED_LENGTH => fp(&["numberAllowedLength"], Codec::EnumText),
            tsk::F_CHECK_UNIQUE => fp(&["checkUnique"], Codec::BoolPresence),
            tsk::F_AUTONUMBERING => fp(&["autonumbering"], Codec::BoolPresence),
            tsk::F_TASK_NUMBER_AUTO_PREFIX => fp(&["taskNumberAutoPrefix"], Codec::EnumText),
            tsk::F_DESCRIPTION_LENGTH => fp(&["descriptionLength"], Codec::IntText),
            tsk::F_ADDRESSING => fp(&["addressing"], Codec::PlainText),
            tsk::F_MAIN_ADDRESSING_ATTRIBUTE => fp(&["mainAddressingAttribute"], Codec::PlainText),
            tsk::F_CURRENT_PERFORMER => fp(&["currentPerformer"], Codec::PlainText),
            tsk::F_BASED_ON => fp(&["basedOn"], Codec::RefList(RefListDialect::Edt)),
            tsk::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &tsk::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            tsk::F_CHARACTERISTICS => fp(&["characteristics"], Codec::Characteristics(CharacteristicsDialect::Edt)),
            tsk::F_DEFAULT_PRESENTATION => fp(&["defaultPresentation"], Codec::EnumText),
            tsk::F_EDIT_TYPE => fp(&["editType"], Codec::EnumText),
            tsk::F_INPUT_BY_STRING => fp(&["inputByString"], Codec::RefList(RefListDialect::Edt)),
            tsk::F_SEARCH_STRING_MODE => fp(&["searchStringModeOnInputByString"], Codec::EnumText),
            tsk::F_FTS_ON_INPUT => fp(&["fullTextSearchOnInputByString"], Codec::EnumText),
            tsk::F_CHOICE_DATA_GET_MODE => fp(&["choiceDataGetModeOnInputByString"], Codec::EnumText),
            tsk::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
            tsk::F_DEFAULT_OBJECT_FORM => fp(&["defaultObjectForm"], Codec::PlainText),
            tsk::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            tsk::F_DEFAULT_CHOICE_FORM => fp(&["defaultChoiceForm"], Codec::PlainText),
            // auxiliary*Form — Designer-only.
            tsk::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
            tsk::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            tsk::F_HELP => fp(&["help"], Codec::HelpConst),
            tsk::F_DATA_LOCK_FIELDS => fp(&["dataLockFields"], Codec::RefList(RefListDialect::Edt)),
            tsk::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            tsk::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            tsk::F_OBJECT_PRESENTATION => fp(&["objectPresentation"], Codec::LocalizedKeyVal),
            tsk::F_EXTENDED_OBJECT_PRESENTATION => fp(&["extendedObjectPresentation"], Codec::LocalizedKeyVal),
            tsk::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            tsk::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            tsk::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            // dataHistory-блок: платформенный EDT НЕСЁТ эти поля (sparse — при не-дефолте).
            tsk::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            tsk::F_UPDATE_DATA_HISTORY => fp(&["updateDataHistoryImmediatelyAfterWrite"], Codec::BoolPresence),
            tsk::F_EXECUTE_AFTER_WRITE_DH => {
                fp(&["executeAfterWriteDataHistoryVersionProcessing"], Codec::BoolPresence)
            }
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let (tag, name_tag) = match collection {
            "Attribute" => ("attributes", "name"),
            "AddressingAttribute" => ("addressingAttributes", "name"),
            "TabularSection" => ("tabularSections", "name"),
            "Command" => ("commands", "name"),
            "Form" => ("forms", "name"),
            "Template" => ("templates", "name"),
            _ => return None,
        };
        Some(ChildLocus { container: &[], child_tag: tag, props_wrapped: false, name_tag, bare_ref: false })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        task_bindings()
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

/// EDT физический порядок свойств корня Task (сверено по корпусу).
static ROOT_ORDER: &[FieldId] = &[
    tsk::F_SYNONYM,
    tsk::F_COMMENT,
    tsk::F_USE_STANDARD_COMMANDS,
    tsk::F_INPUT_BY_STRING,
    tsk::F_SEARCH_STRING_MODE,
    tsk::F_FTS_ON_INPUT,
    tsk::F_CHOICE_DATA_GET_MODE,
    tsk::F_STANDARD_ATTRIBUTES,
    tsk::F_CHARACTERISTICS,
    tsk::F_BASED_ON,
    tsk::F_CREATE_ON_INPUT,
    tsk::F_INCLUDE_HELP_IN_CONTENTS,
    tsk::F_HELP,
    tsk::F_DATA_LOCK_FIELDS,
    tsk::F_DATA_LOCK_CONTROL_MODE,
    tsk::F_FULL_TEXT_SEARCH,
    tsk::F_OBJECT_PRESENTATION,
    tsk::F_LIST_PRESENTATION,
    tsk::F_EXTENDED_LIST_PRESENTATION,
    tsk::F_EXPLANATION,
    // dataHistory — физически ПОСЛЕ presentation-блока, ПЕРЕД numberType (сверено фикстурой).
    tsk::F_DATA_HISTORY,
    tsk::F_NUMBER_TYPE,
    tsk::F_NUMBER_LENGTH,
    tsk::F_NUMBER_ALLOWED_LENGTH,
    tsk::F_CHECK_UNIQUE,
    tsk::F_AUTONUMBERING,
    tsk::F_TASK_NUMBER_AUTO_PREFIX,
    tsk::F_DESCRIPTION_LENGTH,
    tsk::F_ADDRESSING,
    tsk::F_MAIN_ADDRESSING_ATTRIBUTE,
    tsk::F_CURRENT_PERFORMER,
    tsk::F_DEFAULT_PRESENTATION,
    tsk::F_EDIT_TYPE,
    tsk::F_DEFAULT_OBJECT_FORM,
    tsk::F_DEFAULT_LIST_FORM,
    tsk::F_DEFAULT_CHOICE_FORM,
    tsk::F_CHOICE_HISTORY_ON_INPUT,
    // updateDataHistory/executeAfterWrite — физически В КОНЦЕ корня EDT (сверено фикстурой).
    tsk::F_UPDATE_DATA_HISTORY,
    tsk::F_EXECUTE_AFTER_WRITE_DH,
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

/// EDT физический порядок свойств Task.Attribute (root <attributes>).
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

/// EDT физический порядок свойств Task.TabularSection.Attribute.
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
    irc::F_CHOICE_PARAMETER_LINKS,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_CHOICE_HISTORY_ON_INPUT,
    irc::F_FILL_CHECKING,
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

// ===== AddressingAttribute (base + addressingDimension) =====
pub struct EdtAddressingAttribute;
impl LocusMap for EdtAddressingAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == aattr::F_ADDRESSING_DIMENSION {
            return Some(fp(&["addressingDimension"], Codec::PlainText));
        }
        base_lookup(field)
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(ADDRESSING_ATTRIBUTE_ORDER)
    }
}

/// EDT физический порядок свойств Task.AddressingAttribute (сверено по корпусу, блок
/// РольИсполнителя — самый полный): как обычный Attribute, но `dataHistory` поднят ПЕРЕД
/// `fillChecking`/`fillValue` (после choiceHistoryOnInput), а `addressingDimension` —
/// между `indexing` и `fullTextSearch` (который идёт последним).
static ADDRESSING_ATTRIBUTE_ORDER: &[FieldId] = &[
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
    // Порядок сверен фикстурой Задача_ПокрытиеСоставных (топосорт addressingAttributes-блоков):
    // …maxValue fillChecking choiceFoldersAndItems quickChoice createOnInput dataHistory
    // fillFromFillingValue fillValue choiceHistoryOnInput indexing … fullTextSearch.
    irc::F_FILL_CHECKING,
    irc::F_CHOICE_FOLDERS_AND_ITEMS,
    irc::F_CHOICE_PARAMETER_LINKS,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_DATA_HISTORY,
    irc::F_FILL_FROM_FILLING_VALUE,
    irc::F_FILL_VALUE,
    irc::F_CHOICE_HISTORY_ON_INPUT,
    irc::F_INDEXING,
    aattr::F_ADDRESSING_DIMENSION,
    irc::F_FULL_TEXT_SEARCH,
];

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
                    decl: &tsk::STD_ATTRS,
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
            child_spec: task_tabular_section_attribute(),
            child_map: &EdtTabularSectionAttribute,
        }]
    })
}

/// `&'static` бинды child-видов корня Task.
fn task_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: task_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "AddressingAttribute", child_spec: task_addressing_attribute(), child_map: &EdtAddressingAttribute },
            ChildBinding { collection: "TabularSection", child_spec: task_tabular_section(), child_map: &EdtTabularSection },
            ChildBinding { collection: "Form", child_spec: task_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: task_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: task_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Task", spec(), &EdtTask, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Task", spec(), &EdtTask, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    task()
}

/// Строка R+X-харнесса EDT/Task. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Task",
    read,
    write,
    corpus_subpath: "coverage/edt/s3_bizproc/src/Tasks",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
