//! Designer-проекция вида `Task` + его дочерних видов (зеркало
//! `core/spec/metadata/task*.rs`, ARCHITECTURE.md §5; child-objects substrate). ТОЛЬКО
//! размещение/кодировка — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5), включая детей.
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<Task>/<Properties>`, DENSE. Дети
//! Attribute/AddressingAttribute/TabularSection/Command — `<ChildObjects>`+`<Properties>`
//! (props_wrapped=true); TabularSection — recursion-узел. Forms/Templates — BARE-ссылки
//! `<Form>Имя</Form>`. Designer DENSE: эмитит и дефолты; порядок = спек-порядок.

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
use morph1c_core::spec::metadata::task_form_ref::task_form_ref;
use morph1c_core::spec::metadata::task_tabular_section as ts;
use morph1c_core::spec::metadata::task_tabular_section::task_tabular_section;
use morph1c_core::spec::metadata::task_tabular_section_attribute::task_tabular_section_attribute;
use morph1c_core::spec::metadata::task_template_ref::task_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

// Пути от корня <MetaDataObject> через <Task>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["Task", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerTask;

impl LocusMap for DesignerTask {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            tsk::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            tsk::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            tsk::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            tsk::F_NUMBER_TYPE => fp(p!("NumberType"), Codec::EnumText),
            tsk::F_NUMBER_LENGTH => fp(p!("NumberLength"), Codec::IntText),
            tsk::F_NUMBER_ALLOWED_LENGTH => fp(p!("NumberAllowedLength"), Codec::EnumText),
            tsk::F_CHECK_UNIQUE => fp(p!("CheckUnique"), Codec::BoolText),
            tsk::F_AUTONUMBERING => fp(p!("Autonumbering"), Codec::BoolText),
            tsk::F_TASK_NUMBER_AUTO_PREFIX => fp(p!("TaskNumberAutoPrefix"), Codec::EnumText),
            tsk::F_DESCRIPTION_LENGTH => fp(p!("DescriptionLength"), Codec::IntText),
            tsk::F_ADDRESSING => fp(p!("Addressing"), Codec::PlainText),
            tsk::F_MAIN_ADDRESSING_ATTRIBUTE => fp(p!("MainAddressingAttribute"), Codec::PlainText),
            tsk::F_CURRENT_PERFORMER => fp(p!("CurrentPerformer"), Codec::PlainText),
            tsk::F_BASED_ON => fp(p!("BasedOn"), Codec::RefList(RefListDialect::DesignerItem)),
            tsk::F_STANDARD_ATTRIBUTES => fp(
                p!("StandardAttributes"),
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &tsk::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            tsk::F_CHARACTERISTICS => fp(p!("Characteristics"), Codec::Characteristics(CharacteristicsDialect::Designer)),
            tsk::F_DEFAULT_PRESENTATION => fp(p!("DefaultPresentation"), Codec::EnumText),
            tsk::F_EDIT_TYPE => fp(p!("EditType"), Codec::EnumText),
            tsk::F_INPUT_BY_STRING => fp(p!("InputByString"), Codec::RefList(RefListDialect::DesignerField)),
            tsk::F_SEARCH_STRING_MODE => fp(p!("SearchStringModeOnInputByString"), Codec::EnumText),
            tsk::F_FTS_ON_INPUT => fp(p!("FullTextSearchOnInputByString"), Codec::EnumText),
            tsk::F_CHOICE_DATA_GET_MODE => fp(p!("ChoiceDataGetModeOnInputByString"), Codec::EnumText),
            tsk::F_CREATE_ON_INPUT => fp(p!("CreateOnInput"), Codec::EnumText),
            tsk::F_DEFAULT_OBJECT_FORM => fp(p!("DefaultObjectForm"), Codec::PlainText),
            tsk::F_DEFAULT_LIST_FORM => fp(p!("DefaultListForm"), Codec::PlainText),
            tsk::F_DEFAULT_CHOICE_FORM => fp(p!("DefaultChoiceForm"), Codec::PlainText),
            tsk::F_AUX_OBJECT_FORM => fp(p!("AuxiliaryObjectForm"), Codec::PlainText),
            tsk::F_AUX_LIST_FORM => fp(p!("AuxiliaryListForm"), Codec::PlainText),
            tsk::F_AUX_CHOICE_FORM => fp(p!("AuxiliaryChoiceForm"), Codec::PlainText),
            tsk::F_CHOICE_HISTORY_ON_INPUT => fp(p!("ChoiceHistoryOnInput"), Codec::EnumText),
            tsk::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            // help — EDT-only (Designer не несёт): not projected.
            tsk::F_DATA_LOCK_FIELDS => fp(p!("DataLockFields"), Codec::RefList(RefListDialect::DesignerField)),
            tsk::F_DATA_LOCK_CONTROL_MODE => fp(p!("DataLockControlMode"), Codec::EnumText),
            tsk::F_FULL_TEXT_SEARCH => fp(p!("FullTextSearch"), Codec::EnumText),
            tsk::F_OBJECT_PRESENTATION => fp(p!("ObjectPresentation"), Codec::LocalizedV8),
            tsk::F_EXTENDED_OBJECT_PRESENTATION => fp(p!("ExtendedObjectPresentation"), Codec::LocalizedV8),
            tsk::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            tsk::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            tsk::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            tsk::F_DATA_HISTORY => fp(p!("DataHistory"), Codec::EnumText),
            tsk::F_UPDATE_DATA_HISTORY => fp(p!("UpdateDataHistoryImmediatelyAfterWrite"), Codec::BoolText),
            tsk::F_EXECUTE_AFTER_WRITE_DH => fp(p!("ExecuteAfterWriteDataHistoryVersionProcessing"), Codec::BoolText),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["Task", "ChildObjects"];
        let (tag, bare) = match collection {
            "Attribute" => ("Attribute", false),
            "AddressingAttribute" => ("AddressingAttribute", false),
            "TabularSection" => ("TabularSection", false),
            "Command" => ("Command", false),
            "Form" => ("Form", true),
            "Template" => ("Template", true),
            _ => return None,
        };
        Some(ChildLocus { container: cont, child_tag: tag, props_wrapped: !bare, name_tag: "Name", bare_ref: bare })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        task_bindings()
    }

    fn child_emit_order(&self) -> Option<&'static [&'static str]> {
        // Installed SDK BSP witness: Forms precede AddressingAttributes.
        // Other collections retain their relative specification order.
        Some(&["Attribute", "TabularSection", "Form", "AddressingAttribute", "Template", "Command"])
    }

    fn emit_empty_child_container(&self) -> bool {
        // Все Designer-Task несут `<ChildObjects>`.
        true
    }
}

// ===== Дети Attribute / TabularSection.Attribute (ir_child base) =====
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
        irc::F_CHOICE_PARAMETERS => fp(&["ChoiceParameters"], Codec::PlainText),
        irc::F_QUICK_CHOICE => fp(&["QuickChoice"], Codec::EnumText),
        irc::F_CREATE_ON_INPUT => fp(&["CreateOnInput"], Codec::EnumText),
        irc::F_CHOICE_FORM => fp(&["ChoiceForm"], Codec::PlainText),
        irc::F_LINK_BY_TYPE => fp(&["LinkByType"], Codec::PlainText),
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

// ===== AddressingAttribute (base + addressingDimension) =====
pub struct DesignerAddressingAttribute;
impl LocusMap for DesignerAddressingAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == aattr::F_ADDRESSING_DIMENSION {
            return Some(fp(&["AddressingDimension"], Codec::PlainText));
        }
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
                    decl: &tsk::STD_ATTRS,
                    variant: StdAttrsVariant::Tabular,
                },
            ),
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
            child_spec: task_tabular_section_attribute(),
            child_map: &DesignerTabularSectionAttribute,
        }]
    })
}

fn task_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: task_attribute(), child_map: &DesignerAttribute },
            ChildBinding { collection: "AddressingAttribute", child_spec: task_addressing_attribute(), child_map: &DesignerAddressingAttribute },
            ChildBinding { collection: "TabularSection", child_spec: task_tabular_section(), child_map: &DesignerTabularSection },
            ChildBinding { collection: "Form", child_spec: task_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: task_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: task_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Task", spec(), &DesignerTask, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Task", spec(), &DesignerTask, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    task()
}

/// Строка R+X-харнесса Designer/Task. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Task",
    read,
    write,
    corpus_subpath: "coverage/designer/s3_bizproc/Tasks",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
