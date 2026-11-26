//! EDT-проекция вида `ExchangePlan` + его дочерних видов (зеркало
//! `core/spec/metadata/exchange_plan*.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Переиспользует Catalog/Document-
//! субстрат (`ir_child` base-реквизит, ref-list/characteristics/value/type/cpl/picture/
//! help-кодеки, children-рекурсию) + ExchangePlan-дельту (std_attrs_exchange_plan, content,
//! distributedInfoBase/includeConfigurationExtensions).
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дети Attribute/TabularSection/Command — inline
//! (props_wrapped=false); TabularSection — recursion-узел (producedTypes+nested Attribute,
//! stdAttrs Document/Tabular). Forms/Templates — inline-СТАБЫ. `content` — EDT-only
//! multi-sibling (X-исключён). EDT эмитит РАЗРЕЖЁННО; физический порядок свойств —
//! `field_emit_order` (сверено по корпусу).

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};
use morph1c_core::spec::metadata::document as doc;

use morph1c_core::spec::metadata::exchange_plan as ep;
use morph1c_core::spec::metadata::exchange_plan::exchange_plan;
use morph1c_core::spec::metadata::exchange_plan_attribute::exchange_plan_attribute;
use morph1c_core::spec::metadata::exchange_plan_command as cmd;
use morph1c_core::spec::metadata::exchange_plan_command::exchange_plan_command;
use morph1c_core::spec::metadata::exchange_plan_form_ref as fref;
use morph1c_core::spec::metadata::exchange_plan_form_ref::exchange_plan_form_ref;
use morph1c_core::spec::metadata::exchange_plan_tabular_section as ts;
use morph1c_core::spec::metadata::exchange_plan_tabular_section::exchange_plan_tabular_section;
use morph1c_core::spec::metadata::exchange_plan_tabular_section_attribute::exchange_plan_tabular_section_attribute;
use morph1c_core::spec::metadata::exchange_plan_template_ref as tref;
use morph1c_core::spec::metadata::exchange_plan_template_ref::exchange_plan_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ ExchangePlan =====
pub struct EdtExchangePlan;

impl LocusMap for EdtExchangePlan {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ep::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            ep::F_COMMENT => fp(&["comment"], Codec::PlainText),
            ep::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            ep::F_CODE_LENGTH => fp(&["codeLength"], Codec::IntText),
            ep::F_CODE_ALLOWED_LENGTH => fp(&["codeAllowedLength"], Codec::EnumText),
            ep::F_DESCRIPTION_LENGTH => fp(&["descriptionLength"], Codec::IntText),
            ep::F_DEFAULT_PRESENTATION => fp(&["defaultPresentation"], Codec::EnumText),
            ep::F_EDIT_TYPE => fp(&["editType"], Codec::EnumText),
            ep::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::BoolPresence),
            ep::F_CHOICE_MODE => fp(&["choiceMode"], Codec::EnumText),
            ep::F_INPUT_BY_STRING => fp(&["inputByString"], Codec::RefList(RefListDialect::Edt)),
            ep::F_SEARCH_STRING_MODE => fp(&["searchStringModeOnInputByString"], Codec::EnumText),
            ep::F_FTS_ON_INPUT => fp(&["fullTextSearchOnInputByString"], Codec::EnumText),
            ep::F_CHOICE_DATA_GET_MODE => fp(&["choiceDataGetModeOnInputByString"], Codec::EnumText),
            ep::F_DEFAULT_OBJECT_FORM => fp(&["defaultObjectForm"], Codec::PlainText),
            ep::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            ep::F_DEFAULT_CHOICE_FORM => fp(&["defaultChoiceForm"], Codec::PlainText),
            // auxiliary*Form — Designer-only.
            ep::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &ep::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            ep::F_CHARACTERISTICS => fp(&["characteristics"], Codec::Characteristics(CharacteristicsDialect::Edt)),
            ep::F_BASED_ON => fp(&["basedOn"], Codec::RefList(RefListDialect::Edt)),
            ep::F_DISTRIBUTED_INFO_BASE => fp(&["distributedInfoBase"], Codec::BoolPresence),
            ep::F_INCLUDE_CONFIGURATION_EXTENSIONS => fp(&["includeConfigurationExtensions"], Codec::BoolPresence),
            ep::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
            ep::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
            ep::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            ep::F_HELP => fp(&["help"], Codec::HelpConst),
            ep::F_DATA_LOCK_FIELDS => fp(&["dataLockFields"], Codec::RefList(RefListDialect::Edt)),
            ep::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            ep::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            ep::F_OBJECT_PRESENTATION => fp(&["objectPresentation"], Codec::LocalizedKeyVal),
            ep::F_EXTENDED_OBJECT_PRESENTATION => fp(&["extendedObjectPresentation"], Codec::LocalizedKeyVal),
            ep::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            ep::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            ep::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            // dataHistory-блок: платформенный EDT НЕСЁТ эти поля (sparse — при не-дефолте).
            ep::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            ep::F_UPDATE_DATA_HISTORY => fp(&["updateDataHistoryImmediatelyAfterWrite"], Codec::BoolPresence),
            ep::F_EXECUTE_AFTER_WRITE_DH => {
                fp(&["executeAfterWriteDataHistoryVersionProcessing"], Codec::BoolPresence)
            }
            ep::F_CONTENT => fp(&["content"], Codec::ExchangePlanContent),
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
        ep_bindings()
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

/// EDT физический порядок свойств корня ExchangePlan (сверено по корпусу: SSL 1/1 +
/// ERP 16/16, structural-tail T1). Позиции `comment`/`basedOn`/`includeHelpInContents`/
/// `help`/`extended*Presentation`/`explanation`/`quickChoice`/`default*Form`/
/// `distributedInfoBase`/`choiceHistoryOnInput` витнессированы ERP-планами; пара
/// `help`↔`dataLockFields` дополнительно подтверждена BusinessProcess-корпусом
/// (единый EDT-эмиттер: `createOnInput < includeHelpInContents < help < dataLockFields`).
/// Пара `distributedInfoBase`↔`choiceHistoryOnInput` в одном файле не витнессится
/// (ни один план не несёт оба) — порядок между ними корпусом не различим.
static ROOT_ORDER: &[FieldId] = &[
    ep::F_SYNONYM,
    ep::F_COMMENT,
    ep::F_USE_STANDARD_COMMANDS,
    ep::F_INPUT_BY_STRING,
    ep::F_SEARCH_STRING_MODE,
    ep::F_FTS_ON_INPUT,
    ep::F_CHOICE_DATA_GET_MODE,
    ep::F_STANDARD_ATTRIBUTES,
    ep::F_BASED_ON,
    ep::F_CREATE_ON_INPUT,
    ep::F_INCLUDE_HELP_IN_CONTENTS,
    ep::F_HELP,
    ep::F_DATA_LOCK_FIELDS,
    ep::F_DATA_LOCK_CONTROL_MODE,
    ep::F_FULL_TEXT_SEARCH,
    ep::F_OBJECT_PRESENTATION,
    ep::F_EXTENDED_OBJECT_PRESENTATION,
    ep::F_LIST_PRESENTATION,
    ep::F_EXTENDED_LIST_PRESENTATION,
    ep::F_EXPLANATION,
    // dataHistory — физически ПОСЛЕ presentation-блока, ПЕРЕД codeLength (сверено фикстурой).
    ep::F_DATA_HISTORY,
    ep::F_CODE_LENGTH,
    ep::F_CODE_ALLOWED_LENGTH,
    ep::F_DESCRIPTION_LENGTH,
    ep::F_CONTENT,
    ep::F_DEFAULT_PRESENTATION,
    ep::F_EDIT_TYPE,
    ep::F_QUICK_CHOICE,
    ep::F_CHOICE_MODE,
    ep::F_DEFAULT_OBJECT_FORM,
    ep::F_DEFAULT_LIST_FORM,
    ep::F_DEFAULT_CHOICE_FORM,
    ep::F_DISTRIBUTED_INFO_BASE,
    ep::F_CHOICE_HISTORY_ON_INPUT,
    // updateDataHistory/executeAfterWrite — физически В КОНЦЕ корня EDT (сверено фикстурой).
    ep::F_UPDATE_DATA_HISTORY,
    ep::F_EXECUTE_AFTER_WRITE_DH,
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

/// EDT физический порядок свойств ExchangePlan.Attribute (root <attributes>).
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

/// EDT физический порядок свойств ExchangePlan.TabularSection.Attribute.
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
                    decl: &doc::STD_ATTRS,
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
            fref::F_USE_PURPOSES => fp(&["usePurposes"], Codec::RefList(formats_xml::ref_list::RefListDialect::Edt)),
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
            child_spec: exchange_plan_tabular_section_attribute(),
            child_map: &EdtTabularSectionAttribute,
        }]
    })
}

/// `&'static` бинды child-видов корня ExchangePlan.
fn ep_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: exchange_plan_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "TabularSection", child_spec: exchange_plan_tabular_section(), child_map: &EdtTabularSection },
            ChildBinding { collection: "Form", child_spec: exchange_plan_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: exchange_plan_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: exchange_plan_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ExchangePlan", spec(), &EdtExchangePlan, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ExchangePlan", spec(), &EdtExchangePlan, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    exchange_plan()
}

/// Строка R+X-харнесса EDT/ExchangePlan. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "ExchangePlan",
    read,
    write,
    corpus_subpath: "coverage/edt/s3_bizproc/src/ExchangePlans",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
