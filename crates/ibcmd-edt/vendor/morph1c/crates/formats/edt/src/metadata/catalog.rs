//! EDT-проекция вида `Catalog` + его дочерних видов (зеркало
//! `core/spec/metadata/catalog*.rs` + `core/spec/catalog_child.rs`, ARCHITECTURE.md §5).
//! ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec` (§1.6).
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

use morph1c_core::spec::catalog_child as base;
use morph1c_core::spec::metadata::catalog as cat;
use morph1c_core::spec::metadata::catalog::catalog;
use morph1c_core::spec::metadata::catalog_attribute::catalog_attribute;
use morph1c_core::spec::metadata::catalog_command as cmd;
use morph1c_core::spec::metadata::catalog_command::catalog_command;
use morph1c_core::spec::metadata::catalog_form_ref as fref;
use morph1c_core::spec::metadata::catalog_form_ref::catalog_form_ref;
use morph1c_core::spec::metadata::catalog_tabular_section as ts;
use morph1c_core::spec::metadata::catalog_tabular_section::catalog_tabular_section;
use morph1c_core::spec::metadata::catalog_tabular_section_attribute::catalog_tabular_section_attribute;
use morph1c_core::spec::metadata::catalog_template_ref as tref;
use morph1c_core::spec::metadata::catalog_template_ref::catalog_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ Catalog =====
pub struct EdtCatalog;

impl LocusMap for EdtCatalog {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cat::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            cat::F_COMMENT => fp(&["comment"], Codec::PlainText),
            cat::F_HIERARCHICAL => fp(&["hierarchical"], Codec::BoolPresence),
            cat::F_HIERARCHY_TYPE => fp(&["hierarchyType"], Codec::EnumText),
            cat::F_LIMIT_LEVEL_COUNT => fp(&["limitLevelCount"], Codec::BoolPresence),
            cat::F_LEVEL_COUNT => fp(&["levelCount"], Codec::IntText),
            cat::F_FOLDERS_ON_TOP => fp(&["foldersOnTop"], Codec::BoolPresence),
            cat::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            cat::F_OWNERS => fp(&["owners"], Codec::RefList(RefListDialect::Edt)),
            cat::F_SUBORDINATION_USE => fp(&["subordinationUse"], Codec::EnumText),
            cat::F_CODE_LENGTH => fp(&["codeLength"], Codec::IntText),
            cat::F_DESCRIPTION_LENGTH => fp(&["descriptionLength"], Codec::IntText),
            cat::F_CODE_TYPE => fp(&["codeType"], Codec::EnumText),
            cat::F_CODE_ALLOWED_LENGTH => fp(&["codeAllowedLength"], Codec::EnumText),
            cat::F_CODE_SERIES => fp(&["codeSeries"], Codec::EnumText),
            cat::F_CHECK_UNIQUE => fp(&["checkUnique"], Codec::BoolPresence),
            cat::F_AUTONUMBERING => fp(&["autonumbering"], Codec::BoolPresence),
            cat::F_DEFAULT_PRESENTATION => fp(&["defaultPresentation"], Codec::EnumText),
            cat::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &cat::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            cat::F_CHARACTERISTICS => fp(&["characteristics"], Codec::Characteristics(CharacteristicsDialect::Edt)),
            cat::F_PREDEFINED_DATA_UPDATE => fp(&["predefinedDataUpdate"], Codec::EnumText),
            cat::F_EDIT_TYPE => fp(&["editType"], Codec::EnumText),
            cat::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::BoolPresence),
            cat::F_CHOICE_MODE => fp(&["choiceMode"], Codec::EnumText),
            cat::F_INPUT_BY_STRING => fp(&["inputByString"], Codec::RefList(RefListDialect::Edt)),
            cat::F_SEARCH_STRING_MODE => fp(&["searchStringModeOnInputByString"], Codec::EnumText),
            cat::F_FTS_ON_INPUT => fp(&["fullTextSearchOnInputByString"], Codec::EnumText),
            cat::F_CHOICE_DATA_GET_MODE => {
                fp(&["choiceDataGetModeOnInputByString"], Codec::EnumText)
            }
            cat::F_DEFAULT_OBJECT_FORM => fp(&["defaultObjectForm"], Codec::PlainText),
            cat::F_DEFAULT_FOLDER_FORM => fp(&["defaultFolderForm"], Codec::PlainText),
            cat::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            cat::F_DEFAULT_CHOICE_FORM => fp(&["defaultChoiceForm"], Codec::PlainText),
            cat::F_DEFAULT_FOLDER_CHOICE_FORM => fp(&["defaultFolderChoiceForm"], Codec::PlainText),
            // auxiliary{Object,List,Choice}Form платформенный EDT НЕСЁТ (sparse при
            // не-дефолте; witnessed ERP `Catalog.Сценарии` — полный full-ref
            // `Catalog.<Имя>.Form.<Форма>`, тот же формат, что default*Form).
            // UH authentic EDT Catalog.ВидыОтчетов additionally witnesses
            // auxiliaryFolderForm with the same full-reference spelling as Designer.
            // auxiliaryFolderChoiceForm remains unwitnessed and unprojected.
            cat::F_AUX_OBJECT_FORM => fp(&["auxiliaryObjectForm"], Codec::PlainText),
            cat::F_AUX_FOLDER_FORM => fp(&["auxiliaryFolderForm"], Codec::PlainText),
            cat::F_AUX_LIST_FORM => fp(&["auxiliaryListForm"], Codec::PlainText),
            cat::F_AUX_CHOICE_FORM => fp(&["auxiliaryChoiceForm"], Codec::PlainText),
            cat::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            cat::F_HELP => fp(&["help"], Codec::HelpConst),
            cat::F_BASED_ON => fp(&["basedOn"], Codec::RefList(RefListDialect::Edt)),
            cat::F_DATA_LOCK_FIELDS => fp(&["dataLockFields"], Codec::RefList(RefListDialect::Edt)),
            cat::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            cat::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            cat::F_OBJECT_PRESENTATION => fp(&["objectPresentation"], Codec::LocalizedKeyVal),
            cat::F_EXTENDED_OBJECT_PRESENTATION => fp(&["extendedObjectPresentation"], Codec::LocalizedKeyVal),
            cat::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            cat::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            cat::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            cat::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
            cat::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
            // dataHistory-блок: платформенный EDT НЕСЁТ эти поля (sparse — при не-дефолте).
            cat::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            cat::F_UPDATE_DATA_HISTORY => fp(&["updateDataHistoryImmediatelyAfterWrite"], Codec::BoolPresence),
            cat::F_EXECUTE_AFTER_WRITE_DH => {
                fp(&["executeAfterWriteDataHistoryVersionProcessing"], Codec::BoolPresence)
            }
            cat::F_PREDEFINED => fp(&["predefined"], Codec::PredefinedData),
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
        cat_bindings()
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

/// EDT физический порядок свойств корня Catalog (сверено топосортом по корпусу).
static ROOT_ORDER: &[FieldId] = &[
    cat::F_SYNONYM,
    cat::F_COMMENT,
    cat::F_USE_STANDARD_COMMANDS,
    cat::F_INPUT_BY_STRING,
    cat::F_SEARCH_STRING_MODE,
    cat::F_FTS_ON_INPUT,
    cat::F_CHOICE_DATA_GET_MODE,
    cat::F_STANDARD_ATTRIBUTES,
    cat::F_CHARACTERISTICS,
    cat::F_BASED_ON,
    cat::F_CREATE_ON_INPUT,
    cat::F_INCLUDE_HELP_IN_CONTENTS,
    cat::F_HELP,
    cat::F_DATA_LOCK_FIELDS,
    cat::F_DATA_LOCK_CONTROL_MODE,
    cat::F_FULL_TEXT_SEARCH,
    // dataHistory — физически сразу ПОСЛЕ fullTextSearch (сверено фикстурой
    // Спр_ИсторияДанных_Использовать). Companions update/execute — В КОНЦЕ корня (ниже).
    cat::F_DATA_HISTORY,
    cat::F_OBJECT_PRESENTATION,
    cat::F_EXTENDED_OBJECT_PRESENTATION,
    cat::F_LIST_PRESENTATION,
    cat::F_EXTENDED_LIST_PRESENTATION,
    cat::F_EXPLANATION,
    cat::F_HIERARCHICAL,
    cat::F_HIERARCHY_TYPE,
    cat::F_LIMIT_LEVEL_COUNT,
    cat::F_LEVEL_COUNT,
    cat::F_FOLDERS_ON_TOP,
    cat::F_SUBORDINATION_USE,
    cat::F_OWNERS,
    cat::F_CODE_LENGTH,
    cat::F_DESCRIPTION_LENGTH,
    cat::F_CODE_TYPE,
    cat::F_CODE_ALLOWED_LENGTH,
    cat::F_CODE_SERIES,
    cat::F_CHECK_UNIQUE,
    cat::F_AUTONUMBERING,
    cat::F_DEFAULT_PRESENTATION,
    cat::F_PREDEFINED,
    cat::F_PREDEFINED_DATA_UPDATE,
    cat::F_EDIT_TYPE,
    cat::F_QUICK_CHOICE,
    cat::F_CHOICE_MODE,
    cat::F_DEFAULT_OBJECT_FORM,
    cat::F_DEFAULT_FOLDER_FORM,
    cat::F_DEFAULT_LIST_FORM,
    cat::F_DEFAULT_CHOICE_FORM,
    cat::F_DEFAULT_FOLDER_CHOICE_FORM,
    // auxiliary*Form — между defaultFolderChoiceForm и choiceHistoryOnInput (witnessed ERP
    // `Catalog.Сценарии`: defaultChoiceForm → auxObject → auxList → auxChoice; метамодель
    // Catalog: 59 defaultFolderChoiceForm, 60..64 auxiliary*, 65 choiceHistoryOnInput).
    cat::F_AUX_OBJECT_FORM,
    cat::F_AUX_FOLDER_FORM,
    cat::F_AUX_LIST_FORM,
    cat::F_AUX_CHOICE_FORM,
    // choiceHistoryOnInput — ПОСЛЕ defaultFolderChoiceForm (witnessed ERP
    // `Catalog.СтатьиДвиженияДенежныхСредств`: defaultChoiceForm → defaultFolderChoiceForm →
    // choiceHistoryOnInput; ранее стоял между defaultChoiceForm и defaultFolderChoiceForm —
    // ни одна фикстура SSL/coverage не несла оба поля сразу, порядок был недоопределён).
    cat::F_CHOICE_HISTORY_ON_INPUT,
    // dataHistory-companions (update/execute) — физически В КОНЦЕ корня EDT (сверено
    // фикстурами Спр_ОбновлятьИсториюДанных…/Спр_ВыполнятьОбработку…). Сам `dataHistory`
    // размещён ВЫШЕ — сразу после fullTextSearch (блок физически РАЗОРВАН в EDT-схеме).
    cat::F_UPDATE_DATA_HISTORY,
    cat::F_EXECUTE_AFTER_WRITE_DH,
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
        // choiceForm: «Designer-only» было SSL-иллюзией — ERP несёт <choiceForm> на
        // реквизитах (18 значений; путь Kind.Name.Form.F).
        irc::F_CHOICE_FORM => fp(&["choiceForm"], Codec::PlainText),
        irc::F_LINK_BY_TYPE => fp(&["linkByType"], Codec::LinkByType(formats_xml::link_by_type::LinkByTypeDialect::Edt)),
        irc::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
        base::F_USE => fp(&["use"], Codec::EnumText),
        irc::F_INDEXING => fp(&["indexing"], Codec::EnumText),
        irc::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
        irc::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
        _ => return None,
    })
}

/// EDT физический порядок свойств Catalog.Attribute (root <attributes>).
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
    // NB: позиция choiceForm задана ERP-witness'ом (…fillChecking < choiceParameterLinks <
    // choiceForm < fillFromFillingValue…), который ЗАОДНО противоречит SSL-топосорту по паре
    // (fillChecking, cpl) — SSL не имел обоих на одном реквизите, порядок был недоопределён.
    // На ЧТЕНИЕ порядок не влияет (lookup по тегу); перестановку писателя делать только
    // вместе с пере-верификацией SSL-корпуса.
    irc::F_CHOICE_FORM,
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
    irc::F_CHOICE_FORM,
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
                    decl: &cat::STD_ATTRS,
                    variant: StdAttrsVariant::Tabular,
                },
            ),
            ts::F_USE => fp(&["use"], Codec::EnumText),
            // lineNumberLength — ХВОСТОВОЙ лист (физически ПОСЛЕ inline-детей <attributes>,
            // ПЕРЕД <use>; ERP-witnessed недефолт 9; дефолт 5 EDT омитит).
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
        // порядок lineNumberLength < use — ПО XCORE-МЕТАМОДЕЛИ (CatalogTabularSection:
        // [..., attributes, lineNumberLength, use]); co-occurrence в ERP нет
        // (lnl×2, use×11, both×0), корпусом не сверить.
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
            fref::F_FORM_TYPE => fp(&["formType"], Codec::EnumText),
            fref::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            fref::F_COMMENT => fp(&["comment"], Codec::PlainText),
            fref::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            fref::F_HELP => fp(&["help"], Codec::HelpConst),
            // ПЕРЕМЕННЫЙ список сиблингов `<usePurposes>X</usePurposes>` (1..2 witnessed;
            // ERP `Catalog.УбыткиПрошлыхЛет` несёт 3 формы только с PersonalComputer)
            // → RefList-Edt, НЕ UsePurposesConst.
            fref::F_USE_PURPOSES => fp(&["usePurposes"], Codec::RefList(RefListDialect::Edt)),
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
            child_spec: catalog_tabular_section_attribute(),
            child_map: &EdtTabularSectionAttribute,
        }]
    })
}

/// `&'static` бинды child-видов корня Catalog.
fn cat_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: catalog_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "TabularSection", child_spec: catalog_tabular_section(), child_map: &EdtTabularSection },
            ChildBinding { collection: "Form", child_spec: catalog_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: catalog_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: catalog_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Catalog", spec(), &EdtCatalog, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Catalog", spec(), &EdtCatalog, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    catalog()
}

/// Строка R+X-харнесса EDT/Catalog. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Catalog",
    read,
    write,
    corpus_subpath: "coverage/edt/s1_core/src/Catalogs",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
