//! Designer-проекция вида `CalculationRegister` + его дочерних видов (зеркало
//! `core/spec/metadata/calculation_register*.rs`, ARCHITECTURE.md §5). Клон
//! `accumulation_register.rs`, адаптированный под поля регистра РАСЧЁТА: `periodicity`,
//! `actionPeriod`/`basePeriod`, `schedule*`, `chartOfCalculationTypes` (вместо registerType/
//! enableTotalsSplitting).
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<CalculationRegister>/<Properties>`, DENSE.
//! Дети Resource/Attribute/Dimension — `<ChildObjects>` + `<Properties>` (props_wrapped).
//! Наборы полей детей РАЗЛИЧНЫ: Resource (базовый БЕЗ Indexing), Attribute (+ScheduleLink/
//! Indexing), Dimension (+DenyIncompleteValues/BaseDimension/ScheduleLink/Indexing; порядок
//! задаёт `field_emit_order`). standardAttributes — ВАРИАДНЫЙ блок (`Codec::StdAttrs`, 11/0; корпус покрытия — 0-вариант).

use formats_xml::children::ChildBinding;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::calculation_register as cr;
use morph1c_core::spec::metadata::calculation_register_attribute as attr;
use morph1c_core::spec::metadata::calculation_register_attribute::calculation_register_attribute;
use morph1c_core::spec::metadata::calculation_register_command as cmd;
use morph1c_core::spec::metadata::calculation_register_command::calculation_register_command;
use morph1c_core::spec::metadata::calculation_register_dimension as dim;
use morph1c_core::spec::metadata::calculation_register_dimension::calculation_register_dimension;
use morph1c_core::spec::metadata::calculation_register_form_ref::calculation_register_form_ref;
use morph1c_core::spec::metadata::calculation_register_resource::calculation_register_resource;
use morph1c_core::spec::metadata::calculation_register_template_ref::calculation_register_template_ref;
use morph1c_core::spec::ir_child as base;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

// Пути от корня <MetaDataObject> через <CalculationRegister>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["CalculationRegister", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerCalculationRegister;

impl LocusMap for DesignerCalculationRegister {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cr::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            cr::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            cr::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            cr::F_DEFAULT_LIST_FORM => fp(p!("DefaultListForm"), Codec::PlainText),
            cr::F_AUXILIARY_LIST_FORM => fp(p!("AuxiliaryListForm"), Codec::PlainText),
            cr::F_PERIODICITY => fp(p!("Periodicity"), Codec::EnumText),
            cr::F_ACTION_PERIOD => fp(p!("ActionPeriod"), Codec::BoolText),
            cr::F_BASE_PERIOD => fp(p!("BasePeriod"), Codec::BoolText),
            cr::F_SCHEDULE => fp(p!("Schedule"), Codec::PlainText),
            cr::F_SCHEDULE_VALUE => fp(p!("ScheduleValue"), Codec::PlainText),
            cr::F_SCHEDULE_DATE => fp(p!("ScheduleDate"), Codec::PlainText),
            cr::F_CHART_OF_CALCULATION_TYPES => fp(p!("ChartOfCalculationTypes"), Codec::PlainText),
            cr::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            // help — EDT-only (Designer не несёт): not projected.
            // ВАРИАДНЫЙ std-attrs (11/0). МЕЖДУ IncludeHelpInContents и
            // DataLockControlMode (ERP-witnessed Начисления/Удержания) = спек-позиция.
            cr::F_STANDARD_ATTRIBUTES => fp(
                p!("StandardAttributes"),
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &cr::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            cr::F_DATA_LOCK_CONTROL_MODE => fp(p!("DataLockControlMode"), Codec::EnumText),
            cr::F_FULL_TEXT_SEARCH => fp(p!("FullTextSearch"), Codec::EnumText),
            cr::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            cr::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            cr::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["CalculationRegister", "ChildObjects"];
        let (tag, bare) = match collection {
            "Resource" => ("Resource", false),
            "Attribute" => ("Attribute", false),
            "Dimension" => ("Dimension", false),
            "Command" => ("Command", false),
            "Form" => ("Form", true),
            "Template" => ("Template", true),
            _ => return None,
        };
        Some(ChildLocus { container: cont, child_tag: tag, props_wrapped: !bare, name_tag: "Name", bare_ref: bare })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        cr_bindings()
    }
}

// ===== Дети — общий базовый набор (поля, ПРИСУТСТВУЮЩИЕ у Resource/Attribute/Dimension).
// fillFromFillingValue/fillValue/dataHistory/indexing дети регистра расчёта не несут в базе.
fn base_lookup(field: FieldId) -> Option<FieldProjection> {
    Some(match field {
        base::F_SYNONYM => fp(&["Synonym"], Codec::LocalizedV8),
        base::F_COMMENT => fp(&["Comment"], Codec::PlainText),
        base::F_TYPE => fp(&["Type"], Codec::Type(formats_xml::TypeDialect::Designer)),
        base::F_PASSWORD_MODE => fp(&["PasswordMode"], Codec::BoolText),
        base::F_FORMAT => fp(&["Format"], Codec::LocalizedV8),
        base::F_EDIT_FORMAT => fp(&["EditFormat"], Codec::LocalizedV8),
        base::F_TOOL_TIP => fp(&["ToolTip"], Codec::LocalizedV8),
        base::F_MARK_NEGATIVES => fp(&["MarkNegatives"], Codec::BoolText),
        base::F_MASK => fp(&["Mask"], Codec::PlainText),
        base::F_MULTI_LINE => fp(&["MultiLine"], Codec::BoolText),
        base::F_EXTENDED_EDIT => fp(&["ExtendedEdit"], Codec::BoolText),
        base::F_MIN_VALUE => fp(&["MinValue"], Codec::Value(ValueDialect::Designer)),
        base::F_MAX_VALUE => fp(&["MaxValue"], Codec::Value(ValueDialect::Designer)),
        base::F_FILL_CHECKING => fp(&["FillChecking"], Codec::EnumText),
        base::F_CHOICE_FOLDERS_AND_ITEMS => fp(&["ChoiceFoldersAndItems"], Codec::EnumText),
        base::F_CHOICE_PARAMETER_LINKS => fp(
            &["ChoiceParameterLinks"],
            Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Designer),
        ),
        base::F_CHOICE_PARAMETERS => fp(
            &["ChoiceParameters"],
            Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Designer),
        ),
        base::F_QUICK_CHOICE => fp(&["QuickChoice"], Codec::EnumText),
        base::F_CREATE_ON_INPUT => fp(&["CreateOnInput"], Codec::EnumText),
        base::F_CHOICE_FORM => fp(&["ChoiceForm"], Codec::PlainText),
        // linkByType: PlainText-плейсхолдер был SSL-иллюзией (пустые) — ERP несёт
        // структурные значения (DataPath+LinkItem) → структурный кодек.
        base::F_LINK_BY_TYPE => fp(
            &["LinkByType"],
            Codec::LinkByType(formats_xml::link_by_type::LinkByTypeDialect::Designer),
        ),
        base::F_CHOICE_HISTORY_ON_INPUT => fp(&["ChoiceHistoryOnInput"], Codec::EnumText),
        base::F_FULL_TEXT_SEARCH => fp(&["FullTextSearch"], Codec::EnumText),
        _ => return None,
    })
}

/// Resource — базовый набор БЕЗ Indexing.
pub struct DesignerResource;
impl LocusMap for DesignerResource {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
}

/// Attribute — базовый + ScheduleLink + Indexing (перед FullTextSearch).
pub struct DesignerAttribute;
impl LocusMap for DesignerAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            attr::F_SCHEDULE_LINK => fp(&["ScheduleLink"], Codec::PlainText),
            base::F_INDEXING => fp(&["Indexing"], Codec::EnumText),
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(ATTRIBUTE_ORDER)
    }
}

/// Attribute Designer физ.порядок (сверено фикстурой): ScheduleLink → Indexing → FullTextSearch.
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
    base::F_CHOICE_HISTORY_ON_INPUT,
    attr::F_SCHEDULE_LINK,
    base::F_INDEXING,
    base::F_FULL_TEXT_SEARCH,
];

/// Dimension — базовый + DenyIncompleteValues/BaseDimension/ScheduleLink/Indexing.
pub struct DesignerDimension;
impl LocusMap for DesignerDimension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            base::F_DENY_INCOMPLETE_VALUES => fp(&["DenyIncompleteValues"], Codec::BoolText),
            dim::F_BASE_DIMENSION => fp(&["BaseDimension"], Codec::BoolText),
            attr::F_SCHEDULE_LINK => fp(&["ScheduleLink"], Codec::PlainText),
            base::F_INDEXING => fp(&["Indexing"], Codec::EnumText),
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(DIMENSION_ORDER)
    }
}

/// Dimension Designer физ.порядок (сверено фикстурой): DenyIncompleteValues → BaseDimension
/// → ScheduleLink → Indexing → FullTextSearch.
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
    base::F_CHOICE_HISTORY_ON_INPUT,
    base::F_DENY_INCOMPLETE_VALUES,
    dim::F_BASE_DIMENSION,
    attr::F_SCHEDULE_LINK,
    base::F_INDEXING,
    base::F_FULL_TEXT_SEARCH,
];

// ===== Дитя Command (полный sub-object, зеркало InformationRegister.Command) =====
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
            // picture: PlainText-плейсхолдер был SSL-иллюзией (пустые) — ERP несёт ссылки.
            cmd::F_PICTURE => fp(&["Picture"], Codec::PictureRef(formats_xml::picture::PictureDialect::Designer)),
            cmd::F_SHORTCUT => fp(&["Shortcut"], Codec::PlainText),
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

fn cr_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Resource", child_spec: calculation_register_resource(), child_map: &DesignerResource },
            ChildBinding { collection: "Attribute", child_spec: calculation_register_attribute(), child_map: &DesignerAttribute },
            ChildBinding { collection: "Dimension", child_spec: calculation_register_dimension(), child_map: &DesignerDimension },
            ChildBinding { collection: "Form", child_spec: calculation_register_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: calculation_register_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: calculation_register_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::calculation_register::calculation_register;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CalculationRegister", spec(), &DesignerCalculationRegister, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CalculationRegister", spec(), &DesignerCalculationRegister, obj)
        .map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    calculation_register()
}

/// Строка R+X-харнесса Designer/CalculationRegister. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "CalculationRegister",
    read,
    write,
    corpus_subpath: "coverage/designer/s2_registers/CalculationRegisters",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
