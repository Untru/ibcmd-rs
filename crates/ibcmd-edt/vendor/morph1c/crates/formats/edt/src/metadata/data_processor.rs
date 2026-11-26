//! EDT-проекция вида `DataProcessor` + его дочерних видов (зеркало
//! `core/spec/metadata/data_processor*.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка
//! ячеек — канонику держит `core/spec` (§1.6). Переиспользует Catalog/ExchangePlan/Document-
//! субстрат (`ir_child` base-реквизит, value/type/cpl/picture/help-кодеки, children-рекурсию)
//! + DataProcessor-дельту (сокращённый корень БЕЗ standardAttributes/нумерации/представлений).
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дети Attribute/TabularSection/Command — inline
//! (props_wrapped=false); TabularSection — recursion-узел (producedTypes+nested Attribute,
//! stdAttrs Document/Tabular). Forms/Templates — inline-СТАБЫ. EDT эмитит РАЗРЕЖЁННО;
//! физический порядок свойств — `field_emit_order` (сверено по корпусу).

use formats_xml::children::ChildBinding;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};
use morph1c_core::spec::metadata::document as doc;

use morph1c_core::spec::metadata::data_processor as dp;
use morph1c_core::spec::metadata::data_processor::data_processor;
use morph1c_core::spec::metadata::data_processor_attribute::data_processor_attribute;
use morph1c_core::spec::metadata::data_processor_command as cmd;
use morph1c_core::spec::metadata::data_processor_command::data_processor_command;
use morph1c_core::spec::metadata::data_processor_form_ref as fref;
use morph1c_core::spec::metadata::data_processor_form_ref::data_processor_form_ref;
use morph1c_core::spec::metadata::data_processor_tabular_section as ts;
use morph1c_core::spec::metadata::data_processor_tabular_section::data_processor_tabular_section;
use morph1c_core::spec::metadata::data_processor_tabular_section_attribute::data_processor_tabular_section_attribute;
use morph1c_core::spec::metadata::data_processor_template_ref as tref;
use morph1c_core::spec::metadata::data_processor_template_ref::data_processor_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ DataProcessor =====
pub struct EdtDataProcessor;

impl LocusMap for EdtDataProcessor {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            dp::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            dp::F_COMMENT => fp(&["comment"], Codec::PlainText),
            dp::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            dp::F_DEFAULT_FORM => fp(&["defaultForm"], Codec::PlainText),
            dp::F_AUXILIARY_FORM => fp(&["auxiliaryForm"], Codec::PlainText),
            dp::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            dp::F_EXTENDED_PRESENTATION => fp(&["extendedPresentation"], Codec::LocalizedKeyVal),
            dp::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            dp::F_HELP => fp(&["help"], Codec::HelpConst),
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
        dp_bindings()
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

    fn root_extra_namespaces_optional(&self) -> bool {
        // xsi+core объявляются на корне обработки лишь при наличии Attribute/TabularSection.
        true
    }

    fn root_extra_namespaces_present(&self, obj: &morph1c_core::ir::MetadataObject) -> bool {
        // ns нужны ⟺ объект несёт ≥1 дочерний Attribute/TabularSection (их minValue/
        // maxValue/fillValue используют `xsi:type="core:…"`). Сверено по корпусу: 33/78
        // объявляют ns ⟺ 33/78 имеют такие дети. Command/Form/Template ns НЕ требуют.
        obj.children.iter().any(|c| {
            let k = c.kind.as_str();
            k == "DataProcessor.Attribute" || k == "DataProcessor.TabularSection"
        })
    }
}

/// EDT физический порядок свойств корня DataProcessor (сверено по корпусу).
static ROOT_ORDER: &[FieldId] = &[
    dp::F_SYNONYM,
    dp::F_COMMENT,
    dp::F_USE_STANDARD_COMMANDS,
    dp::F_DEFAULT_FORM,
    dp::F_AUXILIARY_FORM,
    dp::F_INCLUDE_HELP_IN_CONTENTS,
    dp::F_HELP,
    dp::F_EXTENDED_PRESENTATION,
    dp::F_EXPLANATION,
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
        // choiceParameters/choiceForm/linkByType — EDT НЕСЁТ (ERP-корпус: 320/15/17
        // непустых у реквизитов обработок; прежняя пометка «Designer-only» — ложная).
        irc::F_CHOICE_PARAMETERS => fp(
            &["choiceParameters"],
            Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Edt),
        ),
        irc::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::EnumText),
        irc::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
        irc::F_CHOICE_FORM => fp(&["choiceForm"], Codec::PlainText),
        irc::F_LINK_BY_TYPE => {
            fp(&["linkByType"], Codec::LinkByType(formats_xml::link_by_type::LinkByTypeDialect::Edt))
        }
        irc::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
        irc::F_INDEXING => fp(&["indexing"], Codec::EnumText),
        irc::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
        irc::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
        _ => return None,
    })
}

/// EDT физический порядок свойств DataProcessor.Attribute (root <attributes>) = порядок
/// фич xcore-класса `DataProcessorAttribute` (BasicFeature + choiceHistoryOnInput в
/// конце). Сверено ERP-корпусом (0 нарушений на 1566 реквизитах): `fillChecking` — ПЕРЕД
/// choice-блоком (обр. ГенерацияТопологииСклада.Помещение: fillChecking<cpl<
/// choiceParameters), mask — между toolTip и minValue
/// (ПодборНомеровГТДВДокументы.РегистрационныйНомер).
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
    irc::F_FILL_CHECKING,
    irc::F_CHOICE_FOLDERS_AND_ITEMS,
    irc::F_CHOICE_PARAMETER_LINKS,
    irc::F_CHOICE_PARAMETERS,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_CHOICE_FORM,
    irc::F_LINK_BY_TYPE,
    irc::F_CHOICE_HISTORY_ON_INPUT,
    // fillFromFillingValue/fillValue/indexing/fullTextSearch/dataHistory — у КОРНЕВОГО
    // реквизита обработки отсутствуют (не DB-объект; см. спек).
];

/// EDT физический порядок свойств DataProcessor.TabularSection.Attribute = порядок фич
/// xcore-класса `DataProcessorTabularSectionAttribute` (…choiceForm<linkByType<
/// fillFromFillingValue<fillValue<choiceHistoryOnInput). Сверено ERP-корпусом (0
/// нарушений на 5558 ТЧ-реквизитах): ВводКорректировкиЗаказаМатериалов.Серия
/// (fillChecking<cpl<choiceParameters<fillValue), ВводФактаПоМаршрутномуЛисту.Назначение
/// (maxValue<choiceForm<fillValue), РедактированиеПоследовательностиЭтаповПроизводства.
/// АналитикаРасходов (cpl<linkByType<fillValue).
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
    irc::F_FILL_CHECKING,
    irc::F_CHOICE_FOLDERS_AND_ITEMS,
    irc::F_CHOICE_PARAMETER_LINKS,
    irc::F_CHOICE_PARAMETERS,
    irc::F_QUICK_CHOICE,
    irc::F_CREATE_ON_INPUT,
    irc::F_CHOICE_FORM,
    irc::F_LINK_BY_TYPE,
    irc::F_FILL_FROM_FILLING_VALUE,
    irc::F_FILL_VALUE,
    irc::F_CHOICE_HISTORY_ON_INPUT,
    // indexing/fullTextSearch/dataHistory — у реквизита табличной части обработки
    // отсутствуют (не DB-объект; см. спек).
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
            // lineNumberLength у ТЧ обработки ОТСУТСТВУЕТ (xcore: DataProcessorTabularSection
            // без этого поля; cf TS body без слота; спек его не объявляет).
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
}

/// EDT физический порядок свойств TabularSection (own-fields).
static TS_ORDER: &[FieldId] =
    &[ts::F_SYNONYM, ts::F_COMMENT, ts::F_TOOL_TIP, ts::F_FILL_CHECKING, ts::F_STANDARD_ATTRIBUTES];

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
            // ПЕРЕМЕННЫЙ список сиблингов `<usePurposes>X</usePurposes>` (1..2 witnessed;
            // ERP `ЕдиныйНалоговыйСчетЛичныйКабинет.Form.ФормаВыбораПериода` несёт только
            // PersonalComputer) → RefList-Edt, НЕ UsePurposesConst.
            fref::F_USE_PURPOSES => fp(&["usePurposes"], Codec::RefList(formats_xml::ref_list::RefListDialect::Edt)),
            fref::F_EXTENDED_PRESENTATION => fp(&["extendedPresentation"], Codec::LocalizedKeyVal),
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
            child_spec: data_processor_tabular_section_attribute(),
            child_map: &EdtTabularSectionAttribute,
        }]
    })
}

/// `&'static` бинды child-видов корня DataProcessor.
fn dp_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: data_processor_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "TabularSection", child_spec: data_processor_tabular_section(), child_map: &EdtTabularSection },
            ChildBinding { collection: "Form", child_spec: data_processor_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: data_processor_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: data_processor_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("DataProcessor", spec(), &EdtDataProcessor, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("DataProcessor", spec(), &EdtDataProcessor, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    data_processor()
}

/// Строка R+X-харнесса EDT/DataProcessor. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "DataProcessor",
    read,
    write,
    corpus_subpath: "coverage/edt/s5_reports_services/src/DataProcessors",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
