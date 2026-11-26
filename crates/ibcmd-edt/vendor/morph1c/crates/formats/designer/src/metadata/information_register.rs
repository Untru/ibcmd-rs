//! Designer-проекция вида `InformationRegister` + его дочерних видов (зеркало
//! `core/spec/metadata/information_register*.rs`, ARCHITECTURE.md §5; child-objects
//! substrate). ТОЛЬКО размещение/кодировка — канонику держит `core/spec` (§1.6). Иной
//! синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5), включая детей.
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<InformationRegister>/<Properties>`,
//! DENSE. Дети Resource/Attribute/Dimension/Command — `<ChildObjects>` + `<Properties>`
//! (props_wrapped=true). Forms/Templates — BARE-ссылки `<Form>Имя</Form>` (bare_ref=true).
//! Designer DENSE: эмитит и дефолты; порядок свойств = спек-порядок (= Designer-порядок).

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
use morph1c_core::spec::metadata::information_register_form_ref::information_register_form_ref;
use morph1c_core::spec::metadata::information_register_resource::information_register_resource;
use morph1c_core::spec::metadata::information_register_template_ref::information_register_template_ref;
use morph1c_core::spec::ir_child as base;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

// Пути от корня <MetaDataObject> через <InformationRegister>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["InformationRegister", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerInformationRegister;

impl LocusMap for DesignerInformationRegister {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ir::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            ir::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            ir::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            ir::F_EDIT_TYPE => fp(p!("EditType"), Codec::EnumText),
            ir::F_DEFAULT_RECORD_FORM => fp(p!("DefaultRecordForm"), Codec::PlainText),
            ir::F_DEFAULT_LIST_FORM => fp(p!("DefaultListForm"), Codec::PlainText),
            ir::F_AUXILIARY_RECORD_FORM => fp(p!("AuxiliaryRecordForm"), Codec::PlainText),
            ir::F_AUXILIARY_LIST_FORM => fp(p!("AuxiliaryListForm"), Codec::PlainText),
            ir::F_STANDARD_ATTRIBUTES => fp(
                p!("StandardAttributes"),
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &ir::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            ir::F_PERIODICITY => fp(p!("InformationRegisterPeriodicity"), Codec::EnumText),
            ir::F_WRITE_MODE => fp(p!("WriteMode"), Codec::EnumText),
            ir::F_MAIN_FILTER_ON_PERIOD => fp(p!("MainFilterOnPeriod"), Codec::BoolText),
            ir::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            // help — EDT-only (Designer не несёт): not projected.
            ir::F_DATA_LOCK_CONTROL_MODE => fp(p!("DataLockControlMode"), Codec::EnumText),
            ir::F_FULL_TEXT_SEARCH => fp(p!("FullTextSearch"), Codec::EnumText),
            ir::F_ENABLE_TOTALS_SLICE_FIRST => fp(p!("EnableTotalsSliceFirst"), Codec::BoolText),
            ir::F_ENABLE_TOTALS_SLICE_LAST => fp(p!("EnableTotalsSliceLast"), Codec::BoolText),
            ir::F_RECORD_PRESENTATION => fp(p!("RecordPresentation"), Codec::LocalizedV8),
            ir::F_EXTENDED_RECORD_PRESENTATION => fp(p!("ExtendedRecordPresentation"), Codec::LocalizedV8),
            ir::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            ir::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            ir::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            ir::F_DATA_HISTORY => fp(p!("DataHistory"), Codec::EnumText),
            ir::F_UPDATE_DATA_HISTORY_IMMEDIATELY => fp(p!("UpdateDataHistoryImmediatelyAfterWrite"), Codec::BoolText),
            ir::F_EXECUTE_AFTER_WRITE_DH => fp(p!("ExecuteAfterWriteDataHistoryVersionProcessing"), Codec::BoolText),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["InformationRegister", "ChildObjects"];
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
        ir_bindings()
    }
}

// ===== Дети Resource/Attribute (base) — пути относительны <Properties> ребёнка =====
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
        base::F_FILL_FROM_FILLING_VALUE => fp(&["FillFromFillingValue"], Codec::BoolText),
        base::F_FILL_VALUE => fp(&["FillValue"], Codec::Value(ValueDialect::Designer)),
        base::F_FILL_CHECKING => fp(&["FillChecking"], Codec::EnumText),
        base::F_CHOICE_FOLDERS_AND_ITEMS => fp(&["ChoiceFoldersAndItems"], Codec::EnumText),
        base::F_CHOICE_PARAMETER_LINKS => fp(
            &["ChoiceParameterLinks"],
            Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Designer),
        ),
        // ChoiceParameters/LinkByType — СТРУКТУРНЫЕ кодеки (как у Catalog), а не
        // PlainText-плейсхолдеры: ERP-корпус несёт их непустыми (cp 12/98/98, lbt 0/7/22
        // у attributes/dimensions/resources).
        base::F_CHOICE_PARAMETERS => fp(
            &["ChoiceParameters"],
            Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Designer),
        ),
        base::F_QUICK_CHOICE => fp(&["QuickChoice"], Codec::EnumText),
        base::F_CREATE_ON_INPUT => fp(&["CreateOnInput"], Codec::EnumText),
        base::F_CHOICE_FORM => fp(&["ChoiceForm"], Codec::PlainText),
        base::F_LINK_BY_TYPE => {
            fp(&["LinkByType"], Codec::LinkByType(formats_xml::link_by_type::LinkByTypeDialect::Designer))
        }
        base::F_CHOICE_HISTORY_ON_INPUT => fp(&["ChoiceHistoryOnInput"], Codec::EnumText),
        base::F_INDEXING => fp(&["Indexing"], Codec::EnumText),
        base::F_FULL_TEXT_SEARCH => fp(&["FullTextSearch"], Codec::EnumText),
        base::F_DATA_HISTORY => fp(&["DataHistory"], Codec::EnumText),
        _ => return None,
    })
}

pub struct DesignerResource;
impl LocusMap for DesignerResource {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
}
pub struct DesignerAttribute;
impl LocusMap for DesignerAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
}

pub struct DesignerDimension;
impl LocusMap for DesignerDimension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if let Some(p) = base_lookup(field) {
            return Some(p);
        }
        Some(match field {
            base::F_MASTER => fp(&["Master"], Codec::BoolText),
            base::F_MAIN_FILTER => fp(&["MainFilter"], Codec::BoolText),
            base::F_DENY_INCOMPLETE_VALUES => fp(&["DenyIncompleteValues"], Codec::BoolText),
            base::F_TYPE_REDUCTION_MODE => fp(&["TypeReductionMode"], Codec::EnumText),
            _ => return None,
        })
    }
}

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
            // picture — спек List (`picture_ref_field`), кодек — PictureRef, как у ВСЕХ
            // остальных Command-видов (catalog_command и др.). РАНЬШЕ стоял PlainText →
            // value-kind mismatch «spec says List, format produced Str» на ERP
            // (`ЗаказыТорговыхПлощадок`: 10 команд с CommonPicture.*/StdPicture.Print).
            cmd::F_PICTURE => fp(&["Picture"], Codec::PictureRef(formats_xml::picture::PictureDialect::Designer)),
            cmd::F_SHORTCUT => fp(&["Shortcut"], Codec::Shortcut),
            cmd::F_ON_MAIN_SERVER_UNAVAILABLE => fp(&["OnMainServerUnavalableBehavior"], Codec::EnumText),
            _ => return None,
        })
    }
}

// FormRef/TemplateRef — bare-ссылки в Designer (LocusMap не проецирует свойств:
// `bare_ref` читает/пишет только имя). Пустая проекция.
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

fn ir_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Resource", child_spec: information_register_resource(), child_map: &DesignerResource },
            ChildBinding { collection: "Attribute", child_spec: information_register_attribute(), child_map: &DesignerAttribute },
            ChildBinding { collection: "Dimension", child_spec: information_register_dimension(), child_map: &DesignerDimension },
            ChildBinding { collection: "Form", child_spec: information_register_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: information_register_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: information_register_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::information_register::information_register;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("InformationRegister", spec(), &DesignerInformationRegister, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("InformationRegister", spec(), &DesignerInformationRegister, obj)
        .map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    information_register()
}

/// Строка R+X-харнесса Designer/InformationRegister. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "InformationRegister",
    read,
    write,
    corpus_subpath: "coverage/designer/s2_registers/InformationRegisters",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
