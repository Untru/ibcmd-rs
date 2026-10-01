//! Designer-проекция вида `AccountingRegister` + его дочерних видов (зеркало
//! `core/spec/metadata/accounting_register*.rs`, ARCHITECTURE.md §5). Клон
//! `accumulation_register.rs`, адаптированный под РЕГИСТР БУХГАЛТЕРИИ:
//! chartOfAccounts/correspondence/periodAdjustmentLength (вместо registerType); дети
//! Resource/Dimension несут balance/accountingFlag(/extDimensionAccountingFlag). Сверено
//! designer-фикстурами `coverage/designer/s2_registers/AccountingRegisters` (15 объектов).
//!
//! РОДИТЕЛЬ — пути через `<AccountingRegister>/<Properties>`, DENSE (spec-порядок совпадает
//! с фикстурой → field_emit_order не нужен). standardAttributes — ВАРИАДНЫЙ блок
//! (`Codec::StdAttrs`; наборы равной длины 12/12/11/0 → NameVar-выбор; корпус покрытия —
//! 0-вариант: `<Properties>` без `<StandardAttributes>`). Дети Resource/Attribute/
//! Dimension/Command — `<ChildObjects>` + `<Properties>`; Forms/Templates — BARE-ссылки.

use formats_xml::children::ChildBinding;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::accounting_register as ar;
use morph1c_core::spec::metadata::accounting_register_attribute::accounting_register_attribute;
use morph1c_core::spec::metadata::accounting_register_command as cmd;
use morph1c_core::spec::metadata::accounting_register_command::accounting_register_command;
use morph1c_core::spec::metadata::accounting_register_dimension::accounting_register_dimension;
use morph1c_core::spec::metadata::accounting_register_form_ref::accounting_register_form_ref;
use morph1c_core::spec::metadata::accounting_register_resource as res;
use morph1c_core::spec::metadata::accounting_register_resource::accounting_register_resource;
use morph1c_core::spec::metadata::accounting_register_template_ref::accounting_register_template_ref;
use morph1c_core::spec::ir_child as base;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

macro_rules! p {
    ($tag:literal) => {
        &["AccountingRegister", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerAccountingRegister;

impl LocusMap for DesignerAccountingRegister {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ar::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            ar::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            ar::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            ar::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            // help — EDT-only (Designer не несёт).
            ar::F_CHART_OF_ACCOUNTS => fp(p!("ChartOfAccounts"), Codec::PlainText),
            ar::F_CORRESPONDENCE => fp(p!("Correspondence"), Codec::BoolText),
            ar::F_PERIOD_ADJUSTMENT_LENGTH => fp(p!("PeriodAdjustmentLength"), Codec::IntText),
            ar::F_DEFAULT_LIST_FORM => fp(p!("DefaultListForm"), Codec::PlainText),
            ar::F_AUXILIARY_LIST_FORM => fp(p!("AuxiliaryListForm"), Codec::PlainText),
            // ВАРИАДНЫЙ std-attrs (NameVar-выбор). МЕЖДУ AuxiliaryListForm и
            // DataLockControlMode (ERP-witnessed все 3 регистра) = спек-позиция.
            ar::F_STANDARD_ATTRIBUTES => fp(
                p!("StandardAttributes"),
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &ar::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            ar::F_DATA_LOCK_CONTROL_MODE => fp(p!("DataLockControlMode"), Codec::EnumText),
            ar::F_ENABLE_TOTALS_SPLITTING => fp(p!("EnableTotalsSplitting"), Codec::BoolText),
            ar::F_FULL_TEXT_SEARCH => fp(p!("FullTextSearch"), Codec::EnumText),
            ar::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            ar::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            ar::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["AccountingRegister", "ChildObjects"];
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
        ac_bindings()
    }
}

// ===== Дети — общий базовый набор (БЕЗ indexing/dataHistory/fill* — как у регистра накопления).
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

/// Resource — базовый + Balance/AccountingFlag/ExtDimensionAccountingFlag (перед FullTextSearch).
pub struct DesignerResource;
impl LocusMap for DesignerResource {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            res::F_BALANCE => fp(&["Balance"], Codec::BoolText),
            res::F_ACCOUNTING_FLAG => fp(&["AccountingFlag"], Codec::PlainText),
            res::F_EXT_DIMENSION_ACCOUNTING_FLAG => fp(&["ExtDimensionAccountingFlag"], Codec::PlainText),
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(RESOURCE_ORDER)
    }
}

/// Attribute — базовый + Indexing (перед FullTextSearch; spec-порядок совпадает → без reorder).
pub struct DesignerAttribute;
impl LocusMap for DesignerAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            base::F_INDEXING => fp(&["Indexing"], Codec::EnumText),
            _ => return None,
        })
    }
}

/// Dimension — базовый + Balance/AccountingFlag/DenyIncompleteValues/Indexing.
pub struct DesignerDimension;
impl LocusMap for DesignerDimension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            res::F_BALANCE => fp(&["Balance"], Codec::BoolText),
            res::F_ACCOUNTING_FLAG => fp(&["AccountingFlag"], Codec::PlainText),
            base::F_DENY_INCOMPLETE_VALUES => fp(&["DenyIncompleteValues"], Codec::BoolText),
            base::F_INDEXING => fp(&["Indexing"], Codec::EnumText),
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(DIMENSION_ORDER)
    }
}

/// Designer физ.порядок Resource: ...choiceHistoryOnInput, Balance, AccountingFlag,
/// ExtDimensionAccountingFlag, FullTextSearch (сверено фикстурой; НЕТ Indexing).
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
    base::F_CHOICE_HISTORY_ON_INPUT,
    res::F_BALANCE,
    res::F_ACCOUNTING_FLAG,
    res::F_EXT_DIMENSION_ACCOUNTING_FLAG,
    base::F_FULL_TEXT_SEARCH,
];

/// Designer физ.порядок Dimension: ...choiceHistoryOnInput, Balance, AccountingFlag,
/// DenyIncompleteValues, Indexing, FullTextSearch (сверено фикстурой).
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
    res::F_BALANCE,
    res::F_ACCOUNTING_FLAG,
    base::F_DENY_INCOMPLETE_VALUES,
    base::F_INDEXING,
    base::F_FULL_TEXT_SEARCH,
];

// ===== Дитя Command (полный sub-object) =====
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

// FormRef/TemplateRef — bare-ссылки.
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

fn ac_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Resource", child_spec: accounting_register_resource(), child_map: &DesignerResource },
            ChildBinding { collection: "Attribute", child_spec: accounting_register_attribute(), child_map: &DesignerAttribute },
            ChildBinding { collection: "Dimension", child_spec: accounting_register_dimension(), child_map: &DesignerDimension },
            ChildBinding { collection: "Form", child_spec: accounting_register_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: accounting_register_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: accounting_register_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::accounting_register::accounting_register;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("AccountingRegister", spec(), &DesignerAccountingRegister, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("AccountingRegister", spec(), &DesignerAccountingRegister, obj)
        .map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    accounting_register()
}

/// Строка R+X-харнесса Designer/AccountingRegister. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "AccountingRegister",
    read,
    write,
    corpus_subpath: "coverage/designer/s2_registers/AccountingRegisters",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
