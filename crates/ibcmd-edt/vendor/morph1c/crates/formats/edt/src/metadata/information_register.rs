//! EDT-проекция вида `InformationRegister` + его дочерних видов (зеркало
//! `core/spec/metadata/information_register*.rs` + `core/spec/ir_child.rs`,
//! ARCHITECTURE.md §5; child-objects substrate). ТОЛЬКО
//! размещение/кодировка ячеек — канонику держит `core/spec` (§1.6).
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дети Resource/Attribute/Dimension/Command —
//! inline `<resources uuid>`/… (props_wrapped=false). Forms/Templates — inline-СТАБЫ
//! (`bare_ref=false`, полный стаб). EDT эмитит РАЗРЕЖЁННО; физический порядок свойств
//! детей расходится с Designer → задаётся `field_emit_order` (сверено по корпусу).

use formats_xml::children::ChildBinding;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::information_register as ir;
use morph1c_core::spec::metadata::information_register_attribute::information_register_attribute;
use morph1c_core::spec::metadata::information_register_command as cmd;
use morph1c_core::spec::metadata::information_register_command::information_register_command;
use morph1c_core::spec::metadata::information_register_dimension::information_register_dimension;
use morph1c_core::spec::metadata::information_register_form_ref as fref;
use morph1c_core::spec::metadata::information_register_form_ref::information_register_form_ref;
use morph1c_core::spec::metadata::information_register_resource::information_register_resource;
use morph1c_core::spec::metadata::information_register_template_ref as tref;
use morph1c_core::spec::metadata::information_register_template_ref::information_register_template_ref;
use morph1c_core::spec::ir_child as base;

/// Плоский локус `PropElement{path:[tag], ns:""}`.
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ InformationRegister =====
pub struct EdtInformationRegister;

impl LocusMap for EdtInformationRegister {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ir::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            ir::F_COMMENT => fp(&["comment"], Codec::PlainText),
            ir::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            ir::F_EDIT_TYPE => fp(&["editType"], Codec::EnumText),
            ir::F_DEFAULT_RECORD_FORM => fp(&["defaultRecordForm"], Codec::PlainText),
            ir::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            // UH authentic EDT InformationRegister.ИзмененныеОбъектыДляВыгрузки
            // carries the same full-reference auxiliary list slot as Designer.
            ir::F_AUXILIARY_LIST_FORM => fp(&["auxiliaryListForm"], Codec::PlainText),
            // UH ИменаФайловИКаталогов also carries the auxiliary record role.
            ir::F_AUXILIARY_RECORD_FORM => fp(&["auxiliaryRecordForm"], Codec::PlainText),
            ir::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &ir::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            ir::F_PERIODICITY => fp(&["informationRegisterPeriodicity"], Codec::EnumText),
            ir::F_WRITE_MODE => fp(&["writeMode"], Codec::EnumText),
            ir::F_MAIN_FILTER_ON_PERIOD => fp(&["mainFilterOnPeriod"], Codec::BoolPresence),
            ir::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            ir::F_HELP => fp(&["help"], Codec::HelpConst),
            ir::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            ir::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            ir::F_ENABLE_TOTALS_SLICE_FIRST => fp(&["enableTotalsSliceFirst"], Codec::BoolPresence),
            ir::F_ENABLE_TOTALS_SLICE_LAST => fp(&["enableTotalsSliceLast"], Codec::BoolPresence),
            ir::F_RECORD_PRESENTATION => fp(&["recordPresentation"], Codec::LocalizedKeyVal),
            ir::F_EXTENDED_RECORD_PRESENTATION => fp(&["extendedRecordPresentation"], Codec::LocalizedKeyVal),
            ir::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            ir::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            ir::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            ir::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            // updateDataHistory*/executeAfterWrite* — EDT НЕСЁТ их при не-дефолте (сверено
            // корпусом покрытия: РегСв_ОбновлятьИсториюДанныхСразуПослеЗаписи_Истина /
            // РегСв_ВыполнятьОбработкуВерсииИсторииДанныхПослеЗаписи_Истина). Дефолт false →
            // разрежённый EDT опускает (presence-bool). Порядок эмиссии = спек-порядок.
            ir::F_UPDATE_DATA_HISTORY_IMMEDIATELY => {
                fp(&["updateDataHistoryImmediatelyAfterWrite"], Codec::BoolPresence)
            }
            ir::F_EXECUTE_AFTER_WRITE_DH => {
                fp(&["executeAfterWriteDataHistoryVersionProcessing"], Codec::BoolPresence)
            }
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let (tag, name_tag) = match collection {
            "Resource" => ("resources", "name"),
            "Attribute" => ("attributes", "name"),
            "Dimension" => ("dimensions", "name"),
            "Command" => ("commands", "name"),
            "Form" => ("forms", "name"),
            "Template" => ("templates", "name"),
            _ => return None,
        };
        Some(ChildLocus { container: &[], child_tag: tag, props_wrapped: false, name_tag, bare_ref: false })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        ir_bindings()
    }

    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(ROOT_ORDER)
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        // EDT IR-корень несёт xmlns:xsi+xmlns:core (нужны Value-xsi и стандартным атрибутам).
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}

/// EDT физический порядок РОДИТЕЛЯ: как спек-порядок, НО `dataHistory` эмитится РАНО —
/// сразу после `comment`, ДО `useStandardCommands` (сверено корпусом покрытия:
/// РегСв_ИсторияДанных_Использовать). Механизм [`XmlSink::ordered`] ставит перечисленные
/// поля по рангу, остальные — в спек-порядке в хвост; поэтому достаточно перечислить
/// [synonym, comment, dataHistory] — прочие ложатся в спек-порядок сами.
static ROOT_ORDER: &[FieldId] = &[ir::F_SYNONYM, ir::F_COMMENT, ir::F_DATA_HISTORY];

// ===== Дети: Resource / Attribute (общий базовый набор) =====
fn base_lookup(field: FieldId) -> Option<FieldProjection> {
    Some(match field {
        base::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
        base::F_COMMENT => fp(&["comment"], Codec::PlainText),
        base::F_TYPE => fp(&["type"], Codec::Type(formats_xml::TypeDialect::Edt)),
        base::F_PASSWORD_MODE => fp(&["passwordMode"], Codec::BoolPresence),
        base::F_FORMAT => fp(&["format"], Codec::LocalizedKeyVal),
        base::F_EDIT_FORMAT => fp(&["editFormat"], Codec::LocalizedKeyVal),
        base::F_TOOL_TIP => fp(&["toolTip"], Codec::LocalizedKeyVal),
        base::F_MARK_NEGATIVES => fp(&["markNegatives"], Codec::BoolPresence),
        base::F_MASK => fp(&["mask"], Codec::PlainText),
        base::F_MULTI_LINE => fp(&["multiLine"], Codec::BoolPresence),
        base::F_EXTENDED_EDIT => fp(&["extendedEdit"], Codec::BoolPresence),
        base::F_MIN_VALUE => fp(&["minValue"], Codec::Value(ValueDialect::Edt)),
        base::F_MAX_VALUE => fp(&["maxValue"], Codec::Value(ValueDialect::Edt)),
        base::F_FILL_FROM_FILLING_VALUE => fp(&["fillFromFillingValue"], Codec::BoolPresence),
        base::F_FILL_VALUE => fp(&["fillValue"], Codec::Value(ValueDialect::Edt)),
        base::F_FILL_CHECKING => fp(&["fillChecking"], Codec::EnumText),
        base::F_CHOICE_FOLDERS_AND_ITEMS => fp(&["choiceFoldersAndItems"], Codec::EnumText),
        base::F_CHOICE_PARAMETER_LINKS => fp(
            &["choiceParameterLinks"],
            Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Edt),
        ),
        // choiceParameters/choiceForm/linkByType — EDT НЕСЁТ (ERP-корпус: cp 12/98/98,
        // choiceForm 4/7/1, linkByType 0/7/22 непустых у attributes/dimensions/resources;
        // прежняя пометка «Designer-only» — ложная, SSL их просто не задавал).
        base::F_CHOICE_PARAMETERS => fp(
            &["choiceParameters"],
            Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Edt),
        ),
        base::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::EnumText),
        base::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
        base::F_CHOICE_FORM => fp(&["choiceForm"], Codec::PlainText),
        base::F_LINK_BY_TYPE => {
            fp(&["linkByType"], Codec::LinkByType(formats_xml::link_by_type::LinkByTypeDialect::Edt))
        }
        base::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
        base::F_INDEXING => fp(&["indexing"], Codec::EnumText),
        base::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
        base::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
        _ => return None,
    })
}

/// EDT физический порядок свойств Resource = ПОЛНЫЙ порядок фич xcore-класса
/// `InformationRegisterResource` (models/metadata/metadata.jsonl): choice-блок между
/// fillChecking и fullTextSearch, хвост …fillValue<choiceHistoryOnInput<indexing.
/// Сверено ERP-корпусом (0 нарушений на 7378 ресурсах): АрендованныеОС.Партнер
/// (fillChecking<choiceParameters), НастройкиУчетаНДС.АналитикаРасходовНеНДС (linkByType),
/// МоделиФормированияСтоимости.Спецификация (cpl<choiceParameters<choiceForm<
/// fullTextSearch). Прежняя линеаризация (cpl<fillChecking; без format/editFormat/
/// choiceFoldersAndItems/choiceHistoryOnInput) была недосвидетельствована SSL-корпусом.
static RESOURCE_ORDER: &[FieldId] = &[
    base::F_SYNONYM,
    base::F_COMMENT,
    base::F_TYPE,
    base::F_PASSWORD_MODE,
    base::F_FORMAT,
    base::F_EDIT_FORMAT,
    base::F_TOOL_TIP,
    base::F_MARK_NEGATIVES,
    base::F_MASK,
    base::F_MULTI_LINE,
    base::F_EXTENDED_EDIT,
    base::F_MIN_VALUE,
    base::F_MAX_VALUE,
    base::F_FILL_CHECKING,
    base::F_CHOICE_FOLDERS_AND_ITEMS,
    base::F_CHOICE_PARAMETER_LINKS,
    base::F_CHOICE_PARAMETERS,
    base::F_QUICK_CHOICE,
    base::F_CREATE_ON_INPUT,
    base::F_CHOICE_FORM,
    base::F_LINK_BY_TYPE,
    base::F_FULL_TEXT_SEARCH,
    base::F_DATA_HISTORY,
    base::F_FILL_FROM_FILLING_VALUE,
    base::F_FILL_VALUE,
    base::F_CHOICE_HISTORY_ON_INPUT,
    base::F_INDEXING,
];

/// EDT физический порядок свойств Attribute = ПОЛНЫЙ порядок фич xcore-класса
/// `InformationRegisterAttribute`: хвост indexing<fullTextSearch<dataHistory<
/// fillFromFillingValue<fillValue<choiceHistoryOnInput ОТЛИЧАЕТСЯ от Resource.
/// Сверено ERP-корпусом (0 нарушений на 2614 реквизитах): ИсходныеДанныеПерерасчетов.
/// КодВычета (maxValue<choiceParameters<fullTextSearch), ВидыАлкогольнойПродукцииЕГАИС.
/// ОКПД2 (toolTip-кластер mask), УдалитьНастройкиЗаполненияСубконтоНаСчетахМеждународного
/// Учета.СтатьяАктивовПассивов (choiceParameters<choiceForm), fillChecking<indexing (x9).
static ATTRIBUTE_ORDER: &[FieldId] = &[
    base::F_SYNONYM,
    base::F_COMMENT,
    base::F_TYPE,
    base::F_PASSWORD_MODE,
    base::F_FORMAT,
    base::F_EDIT_FORMAT,
    base::F_TOOL_TIP,
    base::F_MARK_NEGATIVES,
    base::F_MASK,
    base::F_MULTI_LINE,
    base::F_EXTENDED_EDIT,
    base::F_MIN_VALUE,
    base::F_MAX_VALUE,
    base::F_FILL_CHECKING,
    base::F_CHOICE_FOLDERS_AND_ITEMS,
    base::F_CHOICE_PARAMETER_LINKS,
    base::F_CHOICE_PARAMETERS,
    base::F_QUICK_CHOICE,
    base::F_CREATE_ON_INPUT,
    base::F_CHOICE_FORM,
    base::F_LINK_BY_TYPE,
    base::F_INDEXING,
    base::F_FULL_TEXT_SEARCH,
    base::F_DATA_HISTORY,
    base::F_FILL_FROM_FILLING_VALUE,
    base::F_FILL_VALUE,
    base::F_CHOICE_HISTORY_ON_INPUT,
];

pub struct EdtResource;
impl LocusMap for EdtResource {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(RESOURCE_ORDER)
    }
}

pub struct EdtAttribute;
impl LocusMap for EdtAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(ATTRIBUTE_ORDER)
    }
}

// ===== Дитя Dimension (базовый + master/mainFilter/denyIncompleteValues/typeReductionMode) =====
pub struct EdtDimension;
impl LocusMap for EdtDimension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            base::F_MASTER => fp(&["master"], Codec::BoolPresence),
            base::F_MAIN_FILTER => fp(&["mainFilter"], Codec::BoolPresence),
            base::F_DENY_INCOMPLETE_VALUES => fp(&["denyIncompleteValues"], Codec::BoolPresence),
            base::F_TYPE_REDUCTION_MODE => fp(&["typeReductionMode"], Codec::EnumText),
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(DIMENSION_ORDER)
    }
}

/// EDT физический порядок свойств Dimension = ПОЛНЫЙ порядок фич xcore-класса
/// `InformationRegisterDimension`: fillChecking ПЕРЕД choiceFoldersAndItems и
/// choice-блоком; choiceHistoryOnInput — ПОЗДНО (после fillValue, перед master).
/// Сверено ERP-корпусом (0 нарушений на 4923 измерениях): ВариантыОбеспеченияРаботами.
/// Номенклатура (fillChecking<choiceParameters), ЗначенияКатегорийДляОбластиДанных.
/// ЗначениеКатегорииНовостей (cpl<linkByType), НастройкиЗаполненияСубконтоНаСчетах
/// МеждународногоУчета.ОбъектУчета (choiceForm<master/mainFilter).
static DIMENSION_ORDER: &[FieldId] = &[
    base::F_SYNONYM,
    base::F_COMMENT,
    base::F_TYPE,
    base::F_PASSWORD_MODE,
    base::F_FORMAT,
    base::F_EDIT_FORMAT,
    base::F_TOOL_TIP,
    base::F_MARK_NEGATIVES,
    base::F_MASK,
    base::F_MULTI_LINE,
    base::F_EXTENDED_EDIT,
    base::F_MIN_VALUE,
    base::F_MAX_VALUE,
    base::F_FILL_CHECKING,
    base::F_CHOICE_FOLDERS_AND_ITEMS,
    base::F_CHOICE_PARAMETER_LINKS,
    base::F_CHOICE_PARAMETERS,
    base::F_QUICK_CHOICE,
    base::F_CREATE_ON_INPUT,
    base::F_CHOICE_FORM,
    base::F_LINK_BY_TYPE,
    base::F_DENY_INCOMPLETE_VALUES,
    base::F_INDEXING,
    base::F_FULL_TEXT_SEARCH,
    base::F_DATA_HISTORY,
    base::F_FILL_FROM_FILLING_VALUE,
    base::F_FILL_VALUE,
    base::F_CHOICE_HISTORY_ON_INPUT,
    base::F_MASTER,
    base::F_MAIN_FILTER,
    base::F_TYPE_REDUCTION_MODE,
];

// ===== Дитя Command (полный sub-object) =====
pub struct EdtCommand;
impl LocusMap for EdtCommand {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cmd::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            cmd::F_COMMENT => fp(&["comment"], Codec::PlainText),
            cmd::F_GROUP => fp(&["group"], Codec::EnumText),
            cmd::F_COMMAND_PARAMETER_TYPE => fp(&["commandParameterType"], Codec::Type(formats_xml::TypeDialect::Edt)),
            cmd::F_MODIFIES_DATA => fp(&["modifiesData"], Codec::BoolPresence),
            cmd::F_REPRESENTATION => fp(&["representation"], Codec::EnumText),
            cmd::F_TOOL_TIP => fp(&["toolTip"], Codec::LocalizedKeyVal),
            // picture/shortcut платформенный EDT НЕСЁТ (как у catalog_command; witnessed ERP:
            // `ЗаказыТорговыхПлощадок` — 10 команд с picture CommonPicture.*/StdPicture.Print).
            cmd::F_PICTURE => fp(&["picture"], Codec::PictureRef(formats_xml::picture::PictureDialect::Edt)),
            cmd::F_SHORTCUT => fp(&["shortcut"], Codec::Shortcut),
            // parameterUseMode: «Designer-only» было SSL-иллюзией (ERP-witnessed
            // СезонныеКоэффициенты: между commandParameterType и representation).
            cmd::F_PARAMETER_USE_MODE => fp(&["parameterUseMode"], Codec::EnumText),
            // onMainServer* — Designer-only.
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(COMMAND_ORDER)
    }
}

/// EDT физический порядок Command (сверено; picture/shortcut — в хвосте, как у
/// catalog_command/data_processor_command, witnessed ERP `ЗаказыТорговыхПлощадок`).
static COMMAND_ORDER: &[FieldId] = &[
    cmd::F_SYNONYM,
    cmd::F_GROUP,
    cmd::F_COMMAND_PARAMETER_TYPE,
    cmd::F_PARAMETER_USE_MODE,
    cmd::F_MODIFIES_DATA,
    cmd::F_REPRESENTATION,
    cmd::F_TOOL_TIP,
    cmd::F_PICTURE,
    cmd::F_SHORTCUT,
];

// ===== Дети FormRef / TemplateRef (EDT — полный стаб) =====
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
            // ERP `ЖурналСтатусовФинОтчетностиВБанки.Form.ВыборСтандартногоПериодаГодКвартал`
            // несёт только PersonalComputer) → RefList-Edt, НЕ UsePurposesConst.
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

/// `&'static` бинды child-видов родителя (кэш на процесс).
fn ir_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Resource", child_spec: information_register_resource(), child_map: &EdtResource },
            ChildBinding { collection: "Attribute", child_spec: information_register_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "Dimension", child_spec: information_register_dimension(), child_map: &EdtDimension },
            ChildBinding { collection: "Form", child_spec: information_register_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: information_register_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: information_register_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::information_register::information_register;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("InformationRegister", spec(), &EdtInformationRegister, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("InformationRegister", spec(), &EdtInformationRegister, obj)
        .map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    information_register()
}

/// Строка R+X-харнесса EDT/InformationRegister. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "InformationRegister",
    read,
    write,
    corpus_subpath: "coverage/edt/s2_registers/src/InformationRegisters",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
