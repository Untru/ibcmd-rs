//! EDT-проекция вида `AccumulationRegister` + его дочерних видов (зеркало
//! `core/spec/metadata/accumulation_register*.rs` + `core/spec/ir_child.rs`,
//! ARCHITECTURE.md §5; child-objects substrate). ТОЛЬКО
//! размещение/кодировка ячеек — канонику держит `core/spec` (§1.6).
//!
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. standardAttributes — ВАРИАДНЫЙ блок
//! (`Codec::StdAttrs`; 5/4/0 атрибутов по registerType, выбор по числу блоков). Дети
//! Resource/Attribute/Dimension/Command — inline. Forms/Templates — inline-СТАБЫ.
//! EDT эмитит РАЗРЕЖЁННО; физический порядок свойств детей задаёт `field_emit_order`.

use formats_xml::children::ChildBinding;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::accumulation_register as ar;
use morph1c_core::spec::metadata::accumulation_register_attribute::accumulation_register_attribute;
use morph1c_core::spec::metadata::accumulation_register_command as cmd;
use morph1c_core::spec::metadata::accumulation_register_command::accumulation_register_command;
use morph1c_core::spec::metadata::accumulation_register_dimension as dim;
use morph1c_core::spec::metadata::accumulation_register_dimension::accumulation_register_dimension;
use morph1c_core::spec::metadata::accumulation_register_form_ref as fref;
use morph1c_core::spec::metadata::accumulation_register_form_ref::accumulation_register_form_ref;
use morph1c_core::spec::metadata::accumulation_register_resource::accumulation_register_resource;
use morph1c_core::spec::metadata::accumulation_register_template_ref as tref;
use morph1c_core::spec::metadata::accumulation_register_template_ref::accumulation_register_template_ref;
use morph1c_core::spec::ir_child as base;

/// Плоский локус `PropElement{path:[tag], ns:""}`.
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ AccumulationRegister =====
pub struct EdtAccumulationRegister;

impl LocusMap for EdtAccumulationRegister {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ar::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            ar::F_COMMENT => fp(&["comment"], Codec::PlainText),
            ar::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            ar::F_DEFAULT_LIST_FORM => fp(&["defaultListForm"], Codec::PlainText),
            ar::F_AUXILIARY_LIST_FORM => fp(&["auxiliaryListForm"], Codec::PlainText),
            ar::F_REGISTER_TYPE => fp(&["registerType"], Codec::EnumText),
            ar::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            ar::F_HELP => fp(&["help"], Codec::HelpConst),
            ar::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &ar::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            ar::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            ar::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            ar::F_ENABLE_TOTALS_SPLITTING => fp(&["enableTotalsSplitting"], Codec::BoolPresence),
            ar::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            ar::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            ar::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
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
        ar_bindings()
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        // EDT IR-корень несёт xmlns:xsi+xmlns:core (нужны Value-xsi и стандартным атрибутам).
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}

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

/// EDT физический порядок свойств Resource (сверено топосортом по ERP-корпусу).
static RESOURCE_ORDER: &[FieldId] = &[
    base::F_SYNONYM,
    base::F_COMMENT,
    base::F_TYPE,
    base::F_TOOL_TIP,
    base::F_MARK_NEGATIVES,
    base::F_MIN_VALUE,
    base::F_MAX_VALUE,
    base::F_FILL_CHECKING,
    base::F_FULL_TEXT_SEARCH,
];

/// EDT физический порядок свойств Attribute (сверено топосортом).
static ATTRIBUTE_ORDER: &[FieldId] = &[
    base::F_SYNONYM,
    base::F_COMMENT,
    base::F_TYPE,
    base::F_FORMAT,
    base::F_EDIT_FORMAT,
    base::F_TOOL_TIP,
    base::F_MARK_NEGATIVES,
    base::F_MIN_VALUE,
    base::F_MAX_VALUE,
    base::F_CHOICE_PARAMETERS,
    base::F_FILL_CHECKING,
    base::F_CHOICE_PARAMETER_LINKS,
    base::F_INDEXING,
    base::F_FULL_TEXT_SEARCH,
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

// ===== Дитя Dimension (базовый + denyIncompleteValues + useInTotals) =====
pub struct EdtDimension;
impl LocusMap for EdtDimension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            base::F_DENY_INCOMPLETE_VALUES => fp(&["denyIncompleteValues"], Codec::BoolPresence),
            dim::F_USE_IN_TOTALS => fp(&["useInTotals"], Codec::BoolPresence),
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(DIMENSION_ORDER)
    }
}

/// EDT физический порядок свойств Dimension (сверено топосортом по ERP-корпусу).
static DIMENSION_ORDER: &[FieldId] = &[
    base::F_SYNONYM,
    base::F_COMMENT,
    base::F_TYPE,
    base::F_FORMAT,
    base::F_EDIT_FORMAT,
    base::F_TOOL_TIP,
    base::F_MIN_VALUE,
    base::F_MAX_VALUE,
    base::F_FILL_CHECKING,
    base::F_CHOICE_PARAMETER_LINKS,
    base::F_CHOICE_PARAMETERS,
    base::F_CHOICE_FORM,
    base::F_DENY_INCOMPLETE_VALUES,
    base::F_INDEXING,
    base::F_LINK_BY_TYPE,
    base::F_FULL_TEXT_SEARCH,
    dim::F_USE_IN_TOTALS,
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
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(COMMAND_ORDER)
    }
}

/// EDT физический порядок Command (сверено).
static COMMAND_ORDER: &[FieldId] = &[
    cmd::F_SYNONYM,
    cmd::F_GROUP,
    cmd::F_COMMAND_PARAMETER_TYPE,
    cmd::F_REPRESENTATION,
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

/// `&'static` бинды child-видов родителя (кэш на процесс).
fn ar_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Resource", child_spec: accumulation_register_resource(), child_map: &EdtResource },
            ChildBinding { collection: "Attribute", child_spec: accumulation_register_attribute(), child_map: &EdtAttribute },
            ChildBinding { collection: "Dimension", child_spec: accumulation_register_dimension(), child_map: &EdtDimension },
            ChildBinding { collection: "Form", child_spec: accumulation_register_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: accumulation_register_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: accumulation_register_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса (EDT-only — X не запускается без второго формата) =====
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::accumulation_register::accumulation_register;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("AccumulationRegister", spec(), &EdtAccumulationRegister, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("AccumulationRegister", spec(), &EdtAccumulationRegister, obj)
        .map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    accumulation_register()
}

/// Строка R-харнесса EDT/AccumulationRegister. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "AccumulationRegister",
    read,
    write,
    corpus_subpath: "coverage/edt/s2_registers/src/AccumulationRegisters",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
