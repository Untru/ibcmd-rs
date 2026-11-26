//! Designer-проекция вида `Report` + его дочерних видов (зеркало
//! `core/spec/metadata/report*.rs`, ARCHITECTURE.md §5; child-objects substrate).
//! ТОЛЬКО размещение/кодировка — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5), включая детей. ИСКЛЮЧЕНИЕ: `help` —
//! EDT-only (Designer `.cf` его НЕ несёт) → Designer-проекция его НЕ проецирует; поле
//! X-исключено (`x_ignore`), R равенство IR не нарушает (default false у Designer).
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<Report>/<Properties>`, DENSE (16 полей,
//! сверено 41/41). Дети Attribute/TabularSection/Command — `<ChildObjects>`+`<Properties>`
//! (props_wrapped=true); TabularSection — recursion-узел. Forms/Templates — BARE-ссылки
//! `<Form>Имя</Form>`.

use formats_xml::children::ChildBinding;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};
use morph1c_core::spec::metadata::document as doc;

use morph1c_core::spec::metadata::report as rep;
use morph1c_core::spec::metadata::report::report;
use morph1c_core::spec::metadata::report_attribute::report_attribute;
use morph1c_core::spec::metadata::report_command as cmd;
use morph1c_core::spec::metadata::report_command::report_command;
use morph1c_core::spec::metadata::report_form_ref::report_form_ref;
use morph1c_core::spec::metadata::report_tabular_section as ts;
use morph1c_core::spec::metadata::report_tabular_section::report_tabular_section;
use morph1c_core::spec::metadata::report_tabular_section_attribute::report_tabular_section_attribute;
use morph1c_core::spec::metadata::report_template_ref::report_template_ref;
use morph1c_core::spec::ir_child as irc;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

// Пути от корня <MetaDataObject> через <Report>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["Report", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerReport;

impl LocusMap for DesignerReport {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            rep::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            rep::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            rep::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            rep::F_DEFAULT_FORM => fp(p!("DefaultForm"), Codec::PlainText),
            rep::F_AUXILIARY_FORM => fp(p!("AuxiliaryForm"), Codec::PlainText),
            rep::F_MAIN_DATA_COMPOSITION_SCHEMA => fp(p!("MainDataCompositionSchema"), Codec::PlainText),
            rep::F_DEFAULT_SETTINGS_FORM => fp(p!("DefaultSettingsForm"), Codec::PlainText),
            rep::F_AUXILIARY_SETTINGS_FORM => fp(p!("AuxiliarySettingsForm"), Codec::PlainText),
            rep::F_DEFAULT_VARIANT_FORM => fp(p!("DefaultVariantForm"), Codec::PlainText),
            rep::F_AUXILIARY_VARIANT_FORM => fp(p!("AuxiliaryVariantForm"), Codec::PlainText),
            rep::F_VARIANTS_STORAGE => fp(p!("VariantsStorage"), Codec::PlainText),
            rep::F_SETTINGS_STORAGE => fp(p!("SettingsStorage"), Codec::PlainText),
            rep::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            rep::F_EXTENDED_PRESENTATION => fp(p!("ExtendedPresentation"), Codec::LocalizedV8),
            rep::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            // help — EDT-only (Designer не несёт): not projected.
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["Report", "ChildObjects"];
        let (tag, bare) = match collection {
            "Attribute" => ("Attribute", false),
            "TabularSection" => ("TabularSection", false),
            "Command" => ("Command", false),
            "Form" => ("Form", true),
            "Template" => ("Template", true),
            _ => return None,
        };
        Some(ChildLocus { container: cont, child_tag: tag, props_wrapped: !bare, name_tag: "Name", bare_ref: bare })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        rep_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // ВСЕ Designer-Report несут `<ChildObjects>` (минимум — форма/макет).
        true
    }
}

// ===== Дети Attribute / TabularSection.Attribute (ir_child base) =====
fn base_lookup(field: FieldId) -> Option<FieldProjection> {
    Some(match field {
        irc::F_SYNONYM => fp(&["Synonym"], Codec::LocalizedV8),
        irc::F_COMMENT => fp(&["Comment"], Codec::PlainText),
        irc::F_TYPE => fp(&["Type"], Codec::Type(formats_xml::TypeDialect::Designer)),
        irc::F_PASSWORD_MODE => fp(&["PasswordMode"], Codec::BoolText),
        irc::F_FORMAT => fp(&["Format"], Codec::LocalizedV8),
        irc::F_EDIT_FORMAT => fp(&["EditFormat"], Codec::LocalizedV8),
        irc::F_TOOL_TIP => fp(&["ToolTip"], Codec::LocalizedV8),
        irc::F_MARK_NEGATIVES => fp(&["MarkNegatives"], Codec::BoolText),
        irc::F_MASK => fp(&["Mask"], Codec::PlainText),
        irc::F_MULTI_LINE => fp(&["MultiLine"], Codec::BoolText),
        irc::F_EXTENDED_EDIT => fp(&["ExtendedEdit"], Codec::BoolText),
        irc::F_MIN_VALUE => fp(&["MinValue"], Codec::Value(ValueDialect::Designer)),
        irc::F_MAX_VALUE => fp(&["MaxValue"], Codec::Value(ValueDialect::Designer)),
        irc::F_FILL_FROM_FILLING_VALUE => fp(&["FillFromFillingValue"], Codec::BoolText),
        irc::F_FILL_VALUE => fp(&["FillValue"], Codec::Value(ValueDialect::Designer)),
        irc::F_FILL_CHECKING => fp(&["FillChecking"], Codec::EnumText),
        irc::F_CHOICE_FOLDERS_AND_ITEMS => fp(&["ChoiceFoldersAndItems"], Codec::EnumText),
        irc::F_CHOICE_PARAMETER_LINKS => fp(
            &["ChoiceParameterLinks"],
            Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Designer),
        ),
        // ChoiceParameters/LinkByType — СТРУКТУРНЫЕ кодеки (как у Catalog), а не
        // PlainText-плейсхолдеры: ERP-корпус несёт choiceParameters у отчётов (30).
        irc::F_CHOICE_PARAMETERS => fp(
            &["ChoiceParameters"],
            Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Designer),
        ),
        irc::F_QUICK_CHOICE => fp(&["QuickChoice"], Codec::EnumText),
        irc::F_CREATE_ON_INPUT => fp(&["CreateOnInput"], Codec::EnumText),
        irc::F_CHOICE_FORM => fp(&["ChoiceForm"], Codec::PlainText),
        irc::F_LINK_BY_TYPE => {
            fp(&["LinkByType"], Codec::LinkByType(formats_xml::link_by_type::LinkByTypeDialect::Designer))
        }
        irc::F_CHOICE_HISTORY_ON_INPUT => fp(&["ChoiceHistoryOnInput"], Codec::EnumText),
        irc::F_INDEXING => fp(&["Indexing"], Codec::EnumText),
        irc::F_FULL_TEXT_SEARCH => fp(&["FullTextSearch"], Codec::EnumText),
        irc::F_DATA_HISTORY => fp(&["DataHistory"], Codec::EnumText),
        _ => return None,
    })
}

pub struct DesignerAttribute;
impl LocusMap for DesignerAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
}
pub struct DesignerTabularSectionAttribute;
impl LocusMap for DesignerTabularSectionAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        base_lookup(field)
    }
}

// ===== TabularSection (recursion-узел) =====
pub struct DesignerTabularSection;
impl LocusMap for DesignerTabularSection {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ts::F_SYNONYM => fp(&["Synonym"], Codec::LocalizedV8),
            ts::F_COMMENT => fp(&["Comment"], Codec::PlainText),
            ts::F_TOOL_TIP => fp(&["ToolTip"], Codec::LocalizedV8),
            ts::F_FILL_CHECKING => fp(&["FillChecking"], Codec::EnumText),
            ts::F_STANDARD_ATTRIBUTES => fp(
                &["StandardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &doc::STD_ATTRS,
                    variant: StdAttrsVariant::Tabular,
                },
            ),
            // lineNumberLength — у отчёта ОТСУТСТВУЕТ (нет поля в спеке).
            _ => return None,
        })
    }
    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            "Attribute" => Some(ChildLocus {
                container: &["ChildObjects"],
                child_tag: "Attribute",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }
    fn child_bindings(&self) -> &'static [ChildBinding] {
        ts_bindings()
    }
}

// ===== Command =====
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
            cmd::F_PICTURE => fp(&["Picture"], Codec::PictureRef(formats_xml::picture::PictureDialect::Designer)),
            cmd::F_SHORTCUT => fp(&["Shortcut"], Codec::Shortcut),
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

fn ts_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Attribute",
            child_spec: report_tabular_section_attribute(),
            child_map: &DesignerTabularSectionAttribute,
        }]
    })
}

fn rep_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Attribute", child_spec: report_attribute(), child_map: &DesignerAttribute },
            ChildBinding { collection: "TabularSection", child_spec: report_tabular_section(), child_map: &DesignerTabularSection },
            ChildBinding { collection: "Form", child_spec: report_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: report_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: report_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Report", spec(), &DesignerReport, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Report", spec(), &DesignerReport, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    report()
}

/// Строка R+X-харнесса Designer/Report. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Report",
    read,
    write,
    corpus_subpath: "coverage/designer/s5_reports_services/Reports",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
