//! EDT-проекция вида `ChartOfAccounts` + дочерних видов (зеркало
//! `core/spec/metadata/chart_of_accounts*.rs`, ARCHITECTURE.md §5). Плоские дети корня `.mdo`;
//! дети Attribute/TabularSection/AccountingFlag/ExtDimensionAccountingFlag/Command — inline;
//! Forms/Templates — inline-СТАБЫ. EDT эмитит РАЗРЕЖЁННО; физический порядок свойств задаётся
//! `field_emit_order` (сверено топосортом по корпусу 49 объектов + ERP ChartsOfAccounts ×2).
//! Бухгалтерская специфика: basedOn/extDimensionTypes/maxExtDimensionCount/codeMask/
//! autoOrderByCode/orderLength/codeSeries + СВОЙ std-attrs-блок (10 атрибутов, `coa::STD_ATTRS`)
//! + признаки учёта (accountingFlags/extDimensionAccountingFlags); БЕЗ иерархии.
//!
//! `standardTabularSections` — `Codec::StdTabularSections` (декларация
//! `coa::STD_TABULAR_SECTIONS`; ERP-witnessed). `predefined` — СВОЯ грамматика (≠ Catalog):
//! plain `<code>`, `<accountType>` (литерал, деф. Active), `<offBalance>`, `<order>` c
//! ЗНАЧИМЫМИ пробелами, `<accountingFlags>`-ссылки, `<extDimensionTypes>`-блоки c
//! `<turnover>`, `<childItems>` — кодек `Codec::PredefinedDataCoa`
//! (`formats_xml::predefined_coa`; ERP-witnessed 439 узлов обоих планов).

use formats_xml::characteristics::CharacteristicsDialect;
use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::StdAttrsVariant;
use morph1c_core::spec::common::EntitySpec;

use morph1c_core::spec::metadata::chart_of_accounts as coa;
use morph1c_core::spec::metadata::chart_of_accounts::chart_of_accounts;
use morph1c_core::spec::metadata::chart_of_accounts_accounting_flag::chart_of_accounts_accounting_flag;
use morph1c_core::spec::metadata::chart_of_accounts_attribute::chart_of_accounts_attribute;
use morph1c_core::spec::metadata::chart_of_accounts_ext_dimension_accounting_flag::chart_of_accounts_ext_dimension_accounting_flag;
use morph1c_core::spec::metadata::chart_of_accounts_command as cmd;
use morph1c_core::spec::metadata::chart_of_accounts_command::chart_of_accounts_command;
use morph1c_core::spec::metadata::chart_of_accounts_form_ref as fref;
use morph1c_core::spec::metadata::chart_of_accounts_form_ref::chart_of_accounts_form_ref;
use morph1c_core::spec::metadata::chart_of_accounts_tabular_section as ts;
use morph1c_core::spec::metadata::chart_of_accounts_tabular_section::chart_of_accounts_tabular_section;
use morph1c_core::spec::metadata::chart_of_accounts_tabular_section_attribute::chart_of_accounts_tabular_section_attribute;
use morph1c_core::spec::metadata::chart_of_accounts_template_ref as tref;
use morph1c_core::spec::metadata::chart_of_accounts_template_ref::chart_of_accounts_template_ref;
use morph1c_core::spec::metadata::chart_of_characteristic_types as cct;
use morph1c_core::spec::ir_child as irc;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ ChartOfAccounts =====
pub struct EdtChartOfAccounts;

impl LocusMap for EdtChartOfAccounts {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            coa::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            coa::F_COMMENT => fp(&["comment"], Codec::PlainText),
            coa::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            coa::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            coa::F_BASED_ON => fp(&["basedOn"], Codec::RefList(RefListDialect::Edt)),
            coa::F_EXT_DIMENSION_TYPES => fp(&["extDimensionTypes"], Codec::PlainText),
            coa::F_MAX_EXT_DIMENSION_COUNT => fp(&["maxExtDimensionCount"], Codec::IntText),
            coa::F_CODE_MASK => fp(&["codeMask"], Codec::PlainText),
            coa::F_CODE_LENGTH => fp(&["codeLength"], Codec::IntText),
            coa::F_DESCRIPTION_LENGTH => fp(&["descriptionLength"], Codec::IntText),
            coa::F_CODE_SERIES => fp(&["codeSeries"], Codec::EnumText),
            coa::F_CHECK_UNIQUE => fp(&["checkUnique"], Codec::BoolPresence),
            coa::F_DEFAULT_PRESENTATION => fp(&["defaultPresentation"], Codec::EnumText),
            coa::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &coa::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            coa::F_CHARACTERISTICS => fp(&["characteristics"], Codec::Characteristics(CharacteristicsDialect::Edt)),
            // Стандартная ТЧ ExtDimensionTypes («Виды субконто») с вложенным std-attrs
            // (ERP-witnessed оба объекта; синоним с ПУСТЫМ lang <key></key>).
            coa::F_STANDARD_TABULAR_SECTIONS => fp(
                &["standardTabularSections"],
                Codec::StdTabularSections {
                    dialect: GenDialect::Edt,
                    decl: &coa::STD_TABULAR_SECTIONS,
                },
            ),
            coa::F_PREDEFINED_DATA_UPDATE => fp(&["predefinedDataUpdate"], Codec::EnumText),
            coa::F_EDIT_TYPE => fp(&["editType"], Codec::EnumText),
            coa::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::BoolPresence),
            coa::F_CHOICE_MODE => fp(&["choiceMode"], Codec::EnumText),
            coa::F_INPUT_BY_STRING => fp(&["inputByString"], Codec::RefList(RefListDialect::Edt)),
            coa::F_SEARCH_STRING_MODE => fp(&["searchStringModeOnInputByString"], Codec::EnumText),
            coa::F_FTS_ON_INPUT => fp(&["fullTextSearchOnInputByString"], Codec::EnumText),
            coa::F_CHOICE_DATA_GET_MODE => fp(&["choiceDataGetModeOnInputByString"], Codec::EnumText),
            coa::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
            coa::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
            coa::F_DEFAULT_OBJECT_FORM => fp(&["defaultObjectForm"], Codec::PlainText),
            coa::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            coa::F_DEFAULT_CHOICE_FORM => fp(&["defaultChoiceForm"], Codec::PlainText),
            // auxiliary*Form — Designer-only (EDT не несёт).
            coa::F_AUTO_ORDER_BY_CODE => fp(&["autoOrderByCode"], Codec::BoolPresence),
            coa::F_ORDER_LENGTH => fp(&["orderLength"], Codec::IntText),
            coa::F_DATA_LOCK_FIELDS => fp(&["dataLockFields"], Codec::RefList(RefListDialect::Edt)),
            coa::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            coa::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            coa::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            coa::F_UPDATE_DATA_HISTORY => fp(&["updateDataHistoryImmediatelyAfterWrite"], Codec::BoolPresence),
            coa::F_EXECUTE_AFTER_WRITE_DH => {
                fp(&["executeAfterWriteDataHistoryVersionProcessing"], Codec::BoolPresence)
            }
            coa::F_OBJECT_PRESENTATION => fp(&["objectPresentation"], Codec::LocalizedKeyVal),
            coa::F_EXTENDED_OBJECT_PRESENTATION => fp(&["extendedObjectPresentation"], Codec::LocalizedKeyVal),
            coa::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            coa::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            coa::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            coa::F_HELP => fp(&["help"], Codec::HelpConst),
            // predefined — предопределённые счета (ERP-witnessed 439 узлов; Designer —
            // сайдкар Ext/Predefined.xml, читает pipeline::predefined_read в это же поле).
            coa::F_PREDEFINED => fp(&["predefined"], Codec::PredefinedDataCoa),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let (tag, name_tag) = match collection {
            "Attribute" => ("attributes", "name"),
            "TabularSection" => ("tabularSections", "name"),
            "AccountingFlag" => ("accountingFlags", "name"),
            "ExtDimensionAccountingFlag" => ("extDimensionAccountingFlags", "name"),
            "Command" => ("commands", "name"),
            "Form" => ("forms", "name"),
            "Template" => ("templates", "name"),
            _ => return None,
        };
        Some(ChildLocus { container: &[], child_tag: tag, props_wrapped: false, name_tag, bare_ref: false })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        coa_bindings()
    }

    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(ROOT_ORDER)
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        // `form` — declare-iff-used (writer фильтрует по `tree_uses_prefix`): его несёт
        // ТОЛЬКО AccountType-fillValue std-атрибута Type (`form:AccountTypeValue`) —
        // witnessed Хозрасчетный С xmlns:form, Международный БЕЗ (условность честная).
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
            ("xmlns:form", "http://g5.1c.ru/v8/dt/form"),
        ]
    }
}

/// EDT физический порядок свойств корня ChartOfAccounts (топосорт по корпусу 49/49 +
/// ERP-witnessed Хозрасчетный/Международный). ERP уточнил РЕАЛЬНЫЕ позиции ранее не
/// эмитившихся полей: standardAttributes — между fullTextSearchOnInputByString и
/// createOnInput; extDimensionTypes→maxExtDimensionCount→codeMask — ПОСЛЕ presentations;
/// checkUnique→standardTabularSections→predefined→editType; choiceHistoryOnInput — между
/// choiceMode и defaultObjectForm; autoOrderByCode→orderLength смежны. Остальные
/// не-witnessed позиции сохранены (не эмитятся → на byte-exact не влияют).
static ROOT_ORDER: &[FieldId] = &[
    coa::F_SYNONYM,
    coa::F_COMMENT,
    coa::F_USE_STANDARD_COMMANDS,
    coa::F_INPUT_BY_STRING,
    coa::F_SEARCH_STRING_MODE,
    coa::F_FTS_ON_INPUT,
    coa::F_CHOICE_DATA_GET_MODE,
    coa::F_STANDARD_ATTRIBUTES,
    coa::F_BASED_ON,
    coa::F_CREATE_ON_INPUT,
    coa::F_INCLUDE_HELP_IN_CONTENTS,
    coa::F_HELP,
    coa::F_DATA_LOCK_FIELDS,
    coa::F_DATA_LOCK_CONTROL_MODE,
    coa::F_FULL_TEXT_SEARCH,
    coa::F_OBJECT_PRESENTATION,
    coa::F_EXTENDED_OBJECT_PRESENTATION,
    coa::F_LIST_PRESENTATION,
    coa::F_EXTENDED_LIST_PRESENTATION,
    coa::F_EXPLANATION,
    coa::F_DATA_HISTORY,
    coa::F_EXT_DIMENSION_TYPES,
    coa::F_MAX_EXT_DIMENSION_COUNT,
    coa::F_CODE_MASK,
    coa::F_CODE_LENGTH,
    coa::F_DESCRIPTION_LENGTH,
    coa::F_CODE_SERIES,
    coa::F_CHECK_UNIQUE,
    coa::F_CHARACTERISTICS,
    coa::F_STANDARD_TABULAR_SECTIONS,
    coa::F_PREDEFINED,
    coa::F_PREDEFINED_DATA_UPDATE,
    coa::F_DEFAULT_PRESENTATION,
    coa::F_EDIT_TYPE,
    coa::F_QUICK_CHOICE,
    coa::F_CHOICE_MODE,
    coa::F_CHOICE_HISTORY_ON_INPUT,
    coa::F_DEFAULT_OBJECT_FORM,
    coa::F_DEFAULT_LIST_FORM,
    coa::F_DEFAULT_CHOICE_FORM,
    coa::F_AUTO_ORDER_BY_CODE,
    coa::F_ORDER_LENGTH,
    coa::F_UPDATE_DATA_HISTORY,
    coa::F_EXECUTE_AFTER_WRITE_DH,
];

// ===== Дети Attribute / TabularSection.Attribute (IR base, БЕЗ use) =====
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
        irc::F_INDEXING => fp(&["indexing"], Codec::EnumText),
        irc::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
        irc::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
        _ => return None,
    })
}

/// EDT физический порядок свойств ChartOfAccounts.Attribute (root <attributes>).
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

// ===== AccountingFlag / ExtDimensionAccountingFlag (ERP-witnessed) =====

/// EDT физический порядок свойств признака учёта (ERP-witnessed Международный/Хозрасчетный:
/// synonym → type → toolTip → minValue → maxValue → dataHistory → fillValue; в отличие от
/// Attribute `dataHistory` идёт ПЕРЕД `fillValue`, indexing/fullTextSearch у вида НЕТ).
/// Не-witnessed поля вставлены по образцу ATTRIBUTE_ORDER (не эмитятся при дефолте).
static FLAG_ORDER: &[FieldId] = &[
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
    irc::F_DATA_HISTORY,
    irc::F_FILL_VALUE,
];

pub struct EdtAccountingFlag;
impl LocusMap for EdtAccountingFlag {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(FLAG_ORDER)
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
        // Взаимный порядок lineNumberLength < use — ПО XCORE-МЕТАМОДЕЛИ (как у
        // CatalogTabularSection: [..., attributes, lineNumberLength, use]; сам
        // ChartOfAccountsTabularSection в xcore `use` не несёт — поле объявлено в спеке
        // зеркально Catalog); co-occurrence в корпусах нет, корпусом не сверить.
        &[ts::F_LINE_NUMBER_LENGTH, ts::F_USE]
    }
}

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

fn ts_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Attribute",
            child_spec: chart_of_accounts_tabular_section_attribute(),
            child_map: &EdtTabularSectionAttribute,
        }]
    })
}

fn coa_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: chart_of_accounts_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "TabularSection", child_spec: chart_of_accounts_tabular_section(), child_map: &EdtTabularSection },
            ChildBinding { collection: "AccountingFlag", child_spec: chart_of_accounts_accounting_flag(), child_map: &EdtAccountingFlag },
            ChildBinding { collection: "ExtDimensionAccountingFlag", child_spec: chart_of_accounts_ext_dimension_accounting_flag(), child_map: &EdtAccountingFlag },
            ChildBinding { collection: "Form", child_spec: chart_of_accounts_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: chart_of_accounts_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: chart_of_accounts_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ChartOfAccounts", spec(), &EdtChartOfAccounts, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ChartOfAccounts", spec(), &EdtChartOfAccounts, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    chart_of_accounts()
}

/// Строка R+X-харнесса EDT/ChartOfAccounts. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "ChartOfAccounts",
    read,
    write,
    corpus_subpath: "coverage/edt/s2_registers/src/ChartsOfAccounts",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
