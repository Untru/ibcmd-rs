//! EDT-проекция вида `Document` + его дочерних видов (зеркало
//! `core/spec/metadata/document*.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка
//! ячеек — канонику держит `core/spec` (§1.6). Переиспользует Catalog-субстрат
//! (`ir_child` base-реквизит, ref-list/characteristics/value/type/cpl/picture/help-кодеки,
//! children-рекурсию) + Document-дельту (std_attrs_document, номерные/проводочные поля).
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дети Attribute/TabularSection/Command — inline
//! (props_wrapped=false); TabularSection — recursion-узел (producedTypes+nested
//! Attribute). Forms/Templates — inline-СТАБЫ. EDT эмитит РАЗРЕЖЁННО; физический порядок
//! свойств задаётся `field_emit_order` (сверено топосортом по корпусу).

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::document as doc;
use morph1c_core::spec::metadata::document::document;
use morph1c_core::spec::metadata::document_attribute::document_attribute;
use morph1c_core::spec::metadata::document_command as cmd;
use morph1c_core::spec::metadata::document_command::document_command;
use morph1c_core::spec::metadata::document_form_ref as fref;
use morph1c_core::spec::metadata::document_form_ref::document_form_ref;
use morph1c_core::spec::metadata::document_tabular_section as ts;
use morph1c_core::spec::metadata::document_tabular_section::document_tabular_section;
use morph1c_core::spec::metadata::document_tabular_section_attribute::document_tabular_section_attribute;
use morph1c_core::spec::metadata::document_template_ref as tref;
use morph1c_core::spec::metadata::document_template_ref::document_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ Document =====
pub struct EdtDocument;

impl LocusMap for EdtDocument {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            doc::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            doc::F_COMMENT => fp(&["comment"], Codec::PlainText),
            doc::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            // numerator: «Designer-only» было SSL-иллюзией — ERP несёт ссылку на
            // нумератор (witnessed ВыбытиеДенежныхДокументов: explanation < numerator
            // < numberType).
            doc::F_NUMERATOR => fp(&["numerator"], Codec::PlainText),
            doc::F_NUMBER_TYPE => fp(&["numberType"], Codec::EnumText),
            doc::F_NUMBER_LENGTH => fp(&["numberLength"], Codec::IntText),
            doc::F_NUMBER_ALLOWED_LENGTH => fp(&["numberAllowedLength"], Codec::EnumText),
            doc::F_NUMBER_PERIODICITY => fp(&["numberPeriodicity"], Codec::EnumText),
            doc::F_CHECK_UNIQUE => fp(&["checkUnique"], Codec::BoolPresence),
            doc::F_AUTONUMBERING => fp(&["autonumbering"], Codec::BoolPresence),
            doc::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &doc::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            doc::F_CHARACTERISTICS => fp(&["characteristics"], Codec::Characteristics(CharacteristicsDialect::Edt)),
            doc::F_BASED_ON => fp(&["basedOn"], Codec::RefList(RefListDialect::Edt)),
            doc::F_INPUT_BY_STRING => fp(&["inputByString"], Codec::RefList(RefListDialect::Edt)),
            doc::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
            doc::F_SEARCH_STRING_MODE => fp(&["searchStringModeOnInputByString"], Codec::EnumText),
            doc::F_FTS_ON_INPUT => fp(&["fullTextSearchOnInputByString"], Codec::EnumText),
            doc::F_CHOICE_DATA_GET_MODE => fp(&["choiceDataGetModeOnInputByString"], Codec::EnumText),
            doc::F_DEFAULT_OBJECT_FORM => fp(&["defaultObjectForm"], Codec::PlainText),
            doc::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            doc::F_DEFAULT_CHOICE_FORM => fp(&["defaultChoiceForm"], Codec::PlainText),
            // auxiliary*Form платформенный EDT НЕСЁТ (sparse при не-дефолте; witnessed ERP:
            // Документы АктОРасхожденияхПослеОтгрузки/ВыпускПродукции — полный full-ref
            // `Document.<Имя>.Form.<Форма>`, тот же формат, что default*Form).
            doc::F_AUX_OBJECT_FORM => fp(&["auxiliaryObjectForm"], Codec::PlainText),
            doc::F_AUX_LIST_FORM => fp(&["auxiliaryListForm"], Codec::PlainText),
            doc::F_AUX_CHOICE_FORM => fp(&["auxiliaryChoiceForm"], Codec::PlainText),
            doc::F_POSTING => fp(&["posting"], Codec::EnumText),
            doc::F_REAL_TIME_POSTING => fp(&["realTimePosting"], Codec::EnumText),
            // registerRecordsDeletion/WritingOnPost/sequenceFilling — платформенный EDT НЕСЁТ
            // (sparse при не-дефолте); физически между realTimePosting и registerRecords
            // (сверено Док_УдалениеДвижений/ЗаписьДвижений/ЗаполнениеПоследовательностей).
            doc::F_REGISTER_RECORDS_DELETION => fp(&["registerRecordsDeletion"], Codec::EnumText),
            doc::F_REGISTER_RECORDS_WRITING_ON_POST => fp(&["registerRecordsWritingOnPost"], Codec::EnumText),
            doc::F_SEQUENCE_FILLING => fp(&["sequenceFilling"], Codec::EnumText),
            doc::F_REGISTER_RECORDS => fp(&["registerRecords"], Codec::RefList(RefListDialect::Edt)),
            doc::F_POST_IN_PRIVILEGED_MODE => fp(&["postInPrivilegedMode"], Codec::BoolPresence),
            doc::F_UNPOST_IN_PRIVILEGED_MODE => fp(&["unpostInPrivilegedMode"], Codec::BoolPresence),
            doc::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            doc::F_HELP => fp(&["help"], Codec::HelpConst),
            doc::F_DATA_LOCK_FIELDS => fp(&["dataLockFields"], Codec::RefList(RefListDialect::Edt)),
            doc::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            doc::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            doc::F_OBJECT_PRESENTATION => fp(&["objectPresentation"], Codec::LocalizedKeyVal),
            doc::F_EXTENDED_OBJECT_PRESENTATION => fp(&["extendedObjectPresentation"], Codec::LocalizedKeyVal),
            doc::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            doc::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            doc::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            doc::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
            // dataHistory-блок: платформенный EDT НЕСЁТ эти поля (sparse — при не-дефолте).
            // Физика: `dataHistory` сразу после fullTextSearch; companions update/execute —
            // В КОНЦЕ корня (сверено фикстурами Док_ИсторияДанных/Обновлять/Выполнять).
            doc::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            doc::F_UPDATE_DATA_HISTORY => fp(&["updateDataHistoryImmediatelyAfterWrite"], Codec::BoolPresence),
            doc::F_EXECUTE_AFTER_WRITE_DH => {
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
        doc_bindings()
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

/// EDT физический порядок свойств корня Document (сверено топосортом по корпусу).
static ROOT_ORDER: &[FieldId] = &[
    doc::F_SYNONYM,
    doc::F_COMMENT,
    doc::F_USE_STANDARD_COMMANDS,
    doc::F_INPUT_BY_STRING,
    // searchStringModeOnInputByString — сразу после inputByString, перед fullTextSearchOnInputByString
    // (сверено Док_РежимПоискаСтрокиПриВводеПоСтроке_ЛюбаяЧасть).
    doc::F_SEARCH_STRING_MODE,
    doc::F_FTS_ON_INPUT,
    // choiceDataGetModeOnInputByString — сразу после fullTextSearchOnInputByString (зеркало
    // Catalog; сверено Док_РежимПолученияДанныхВыбораПриВводеПоСтроке_ВФоне).
    doc::F_CHOICE_DATA_GET_MODE,
    doc::F_STANDARD_ATTRIBUTES,
    doc::F_CHARACTERISTICS,
    doc::F_BASED_ON,
    doc::F_CREATE_ON_INPUT,
    // includeHelpInContents — физически сразу ПОСЛЕ createOnInput, перед help (сверено
    // фикстурой Док_ВключатьВСодержаниеСправки_Истина; зеркало Catalog-порядка).
    doc::F_INCLUDE_HELP_IN_CONTENTS,
    doc::F_HELP,
    doc::F_DATA_LOCK_CONTROL_MODE,
    doc::F_FULL_TEXT_SEARCH,
    // dataHistory — физически сразу ПОСЛЕ fullTextSearch (сверено Док_ИсторияДанных_Использовать).
    // Companions update/execute — В КОНЦЕ корня (ниже).
    doc::F_DATA_HISTORY,
    doc::F_OBJECT_PRESENTATION,
    doc::F_LIST_PRESENTATION,
    doc::F_EXTENDED_LIST_PRESENTATION,
    doc::F_EXPLANATION,
    doc::F_NUMERATOR,
    doc::F_NUMBER_TYPE,
    doc::F_NUMBER_LENGTH,
    doc::F_NUMBER_ALLOWED_LENGTH,
    doc::F_NUMBER_PERIODICITY,
    doc::F_CHECK_UNIQUE,
    doc::F_AUTONUMBERING,
    doc::F_DEFAULT_OBJECT_FORM,
    doc::F_DEFAULT_LIST_FORM,
    doc::F_DEFAULT_CHOICE_FORM,
    // auxiliary*Form — между defaultChoiceForm и posting/realTimePosting (witnessed ERP
    // ВыпускПродукции: defaultChoiceForm → auxObject → auxList → auxChoice → realTimePosting;
    // метамодель Document: 44 defaultChoiceForm, 45..47 auxiliary*, 48 posting).
    doc::F_AUX_OBJECT_FORM,
    doc::F_AUX_LIST_FORM,
    doc::F_AUX_CHOICE_FORM,
    doc::F_POSTING,
    doc::F_REAL_TIME_POSTING,
    // Движения проведения: registerRecordsDeletion/WritingOnPost/sequenceFilling —
    // между realTimePosting и registerRecords (Designer-DENSE-порядок; сверено фикстурами).
    doc::F_REGISTER_RECORDS_DELETION,
    doc::F_REGISTER_RECORDS_WRITING_ON_POST,
    doc::F_SEQUENCE_FILLING,
    doc::F_REGISTER_RECORDS,
    doc::F_POST_IN_PRIVILEGED_MODE,
    doc::F_UNPOST_IN_PRIVILEGED_MODE,
    doc::F_DATA_LOCK_FIELDS,
    doc::F_CHOICE_HISTORY_ON_INPUT,
    // dataHistory-companions (update/execute) — физически В КОНЦЕ корня EDT (сверено
    // фикстурами Док_ОбновлятьИсториюДанных…/Док_ВыполнятьОбработку…). Сам `dataHistory` —
    // выше, сразу после fullTextSearch.
    doc::F_UPDATE_DATA_HISTORY,
    doc::F_EXECUTE_AFTER_WRITE_DH,
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
        // choiceParameters/choiceForm/linkByType — EDT НЕСЁТ (ERP-корпус: 4651/52/345
        // непустых у Document-реквизитов; прежняя пометка «Designer-only» — ложная,
        // SSL их просто не задавал).
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

/// EDT физический порядок свойств Document.Attribute (root <attributes>) = ПОЛНЫЙ
/// порядок фич xcore-класса `DocumentAttribute` (models/metadata/metadata.jsonl).
/// Сверено ERP-корпусом (0 нарушений на 17890 реквизитах): `fillChecking` — ПЕРЕД
/// choice-блоком (Док ЧекККМ.ВидЦены: fillChecking<cpl<choiceParameters), `choiceForm`/
/// `linkByType` — ПОСЛЕ createOnInput (ВходящаяТранспортнаяОперацияВЕТИС.
/// ГрузоотправительПредприятие / ВводОстатковНМАМеждународныйУчет.АналитикаРасходов),
/// `choiceHistoryOnInput` — ПОСЛЕДНИМ, после dataHistory (x64). Прежняя линеаризация
/// (cpl<fillChecking, chi ранним) была недосвидетельствована SSL-корпусом.
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
    irc::F_FILL_FROM_FILLING_VALUE,
    irc::F_FILL_VALUE,
    irc::F_INDEXING,
    irc::F_FULL_TEXT_SEARCH,
    irc::F_DATA_HISTORY,
    irc::F_CHOICE_HISTORY_ON_INPUT,
];

/// EDT физический порядок свойств Document.TabularSection.Attribute = порядок фич
/// xcore-класса `TabularSectionAttribute`: хвост dataHistory<choiceHistoryOnInput<
/// indexing<fullTextSearch ОТЛИЧАЕТСЯ от корневого реквизита. Сверено ERP-корпусом
/// (0 нарушений на 24131 ТЧ-реквизите; напр. АвансовыйОтчет.АналитикаРасходов:
/// fillChecking<cpl<linkByType<dataHistory).
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
    irc::F_DATA_HISTORY,
    irc::F_CHOICE_HISTORY_ON_INPUT,
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
            // ERP witness мирКонсолидированнаяИнвентаризация = 9; дефолт 5 EDT омитит).
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
        // EDT: <lineNumberLength> физически ПОСЛЕ inline-детей <attributes> (ERP-witnessed).
        &[ts::F_LINE_NUMBER_LENGTH]
    }
}

/// EDT физический порядок свойств TabularSection (own-fields, сверено топосортом).
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
            child_spec: document_tabular_section_attribute(),
            child_map: &EdtTabularSectionAttribute,
        }]
    })
}

/// `&'static` бинды child-видов корня Document.
fn doc_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: document_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "TabularSection", child_spec: document_tabular_section(), child_map: &EdtTabularSection },
            ChildBinding { collection: "Form", child_spec: document_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: document_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: document_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Document", spec(), &EdtDocument, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Document", spec(), &EdtDocument, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    document()
}

/// Строка R+X-харнесса EDT/Document. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Document",
    read,
    write,
    corpus_subpath: "coverage/edt/s1_core/src/Documents",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
