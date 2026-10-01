//! EDT-проекция вида `CalculationRegister` + его дочерних видов (зеркало
//! `core/spec/metadata/calculation_register*.rs` + `core/spec/ir_child.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дети Resource/Attribute/Dimension — inline.
//! EDT эмитит РАЗРЕЖЁННО (physical order = подмножество спек-порядка; для witnessed-полей
//! coverage-корпуса спек-порядок совпадает → field_emit_order не нужен). Расчёт-специфичные
//! поля детей (scheduleLink/baseDimension/denyIncompleteValues/indexing) в корпусе покрытия
//! дефолтны → не эмитятся.

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
use morph1c_core::spec::metadata::calculation_register_form_ref as fref;
use morph1c_core::spec::metadata::calculation_register_form_ref::calculation_register_form_ref;
use morph1c_core::spec::metadata::calculation_register_resource::calculation_register_resource;
use morph1c_core::spec::metadata::calculation_register_template_ref as tref;
use morph1c_core::spec::metadata::calculation_register_template_ref::calculation_register_template_ref;
use morph1c_core::spec::ir_child as base;

/// Плоский локус `PropElement{path:[tag], ns:""}`.
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ CalculationRegister =====
pub struct EdtCalculationRegister;

impl LocusMap for EdtCalculationRegister {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cr::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            cr::F_COMMENT => fp(&["comment"], Codec::PlainText),
            cr::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            cr::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            cr::F_AUXILIARY_LIST_FORM => fp(&["auxiliaryListForm"], Codec::PlainText),
            cr::F_PERIODICITY => fp(&["periodicity"], Codec::EnumText),
            cr::F_ACTION_PERIOD => fp(&["actionPeriod"], Codec::BoolPresence),
            cr::F_BASE_PERIOD => fp(&["basePeriod"], Codec::BoolPresence),
            cr::F_SCHEDULE => fp(&["schedule"], Codec::PlainText),
            cr::F_SCHEDULE_VALUE => fp(&["scheduleValue"], Codec::PlainText),
            cr::F_SCHEDULE_DATE => fp(&["scheduleDate"], Codec::PlainText),
            cr::F_CHART_OF_CALCULATION_TYPES => fp(&["chartOfCalculationTypes"], Codec::PlainText),
            cr::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            cr::F_HELP => fp(&["help"], Codec::HelpConst),
            // ВАРИАДНЫЙ std-attrs (11/0). Физически ПОСЛЕ chartOfCalculationTypes
            // (ERP-witnessed Начисления/Удержания) = спек-позиция после help.
            cr::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &cr::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            cr::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            cr::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            cr::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            cr::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            cr::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let (tag, name_tag) = match collection {
            "Resource" => ("resources", "name"),
            "Attribute" => ("attributes", "name"),
            "Dimension" => ("dimensions", "name"),
            "Form" => ("forms", "name"),
            "Template" => ("templates", "name"),
            "Command" => ("commands", "name"),
            _ => return None,
        };
        Some(ChildLocus { container: &[], child_tag: tag, props_wrapped: false, name_tag, bare_ref: false })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        cr_bindings()
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}

// ===== Дети: общий базовый набор =====
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

pub struct EdtResource;
impl LocusMap for EdtResource {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
}

pub struct EdtAttribute;
impl LocusMap for EdtAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            attr::F_SCHEDULE_LINK => fp(&["scheduleLink"], Codec::PlainText),
            _ => return None,
        })
    }
}

pub struct EdtDimension;
impl LocusMap for EdtDimension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            base::F_DENY_INCOMPLETE_VALUES => fp(&["denyIncompleteValues"], Codec::BoolPresence),
            dim::F_BASE_DIMENSION => fp(&["baseDimension"], Codec::BoolPresence),
            attr::F_SCHEDULE_LINK => fp(&["scheduleLink"], Codec::PlainText),
            _ => return None,
        })
    }
}

// ===== Дети FormRef / TemplateRef (EDT — полный inline-стаб) =====
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

// ===== Дитя Command (полный sub-object, зеркало InformationRegister.Command) =====
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
            // parameterUseMode/picture/shortcut/onMainServer* — Designer-only.
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(COMMAND_ORDER)
    }
}

/// EDT физический порядок Command (зеркало InformationRegister.Command; сверено s15).
static COMMAND_ORDER: &[FieldId] = &[
    cmd::F_SYNONYM,
    cmd::F_GROUP,
    cmd::F_COMMAND_PARAMETER_TYPE,
    cmd::F_MODIFIES_DATA,
    cmd::F_REPRESENTATION,
    cmd::F_TOOL_TIP,
];

/// `&'static` бинды child-видов родителя (кэш на процесс).
fn cr_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Resource", child_spec: calculation_register_resource(), child_map: &EdtResource },
            ChildBinding { collection: "Attribute", child_spec: calculation_register_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "Dimension", child_spec: calculation_register_dimension(), child_map: &EdtDimension },
            ChildBinding { collection: "Form", child_spec: calculation_register_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: calculation_register_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: calculation_register_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R-харнесса (EDT; X сверяет с Designer) =====
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::calculation_register::calculation_register;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CalculationRegister", spec(), &EdtCalculationRegister, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CalculationRegister", spec(), &EdtCalculationRegister, obj)
        .map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    calculation_register()
}

/// Строка R+X-харнесса EDT/CalculationRegister. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "CalculationRegister",
    read,
    write,
    corpus_subpath: "coverage/edt/s2_registers/src/CalculationRegisters",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
