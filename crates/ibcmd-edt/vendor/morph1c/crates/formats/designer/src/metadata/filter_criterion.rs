//! Designer-проекция вида `FilterCriterion` + его дочерних видов (зеркало
//! `core/spec/metadata/filter_criterion*.rs`, ARCHITECTURE.md §5; child-objects substrate).
//! ТОЛЬКО размещение/кодировка — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5), включая детей. Платформенный
//! `<InternalInfo>` (List+Manager) фреймится каркасом коннектора (`PRODUCED_CATEGORIES`).
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<FilterCriterion>/<Properties>`, DENSE
//! (10 полей, сверено ERP 10/10). `Content` — контейнер `<xr:Item xsi:type="xr:MDObjectRef">`
//! (RefList item-style). Forms — BARE-ссылки `<Form>Имя</Form>`; Commands —
//! `<ChildObjects>`+`<Properties>` (props_wrapped, = Report.Command).

use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;

use morph1c_core::spec::metadata::filter_criterion as fc;
use morph1c_core::spec::metadata::filter_criterion::filter_criterion;
use morph1c_core::spec::metadata::filter_criterion_command as cmd;
use morph1c_core::spec::metadata::filter_criterion_command::filter_criterion_command;
use morph1c_core::spec::metadata::filter_criterion_form_ref::filter_criterion_form_ref;

const fn elem(path: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path, ns: "" }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path), codec }
}

// Пути от корня <MetaDataObject> через <FilterCriterion>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["FilterCriterion", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerFilterCriterion;

impl LocusMap for DesignerFilterCriterion {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            fc::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            fc::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            fc::F_TYPE => fp(p!("Type"), Codec::Type(formats_xml::TypeDialect::Designer)),
            fc::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            fc::F_CONTENT => fp(p!("Content"), Codec::RefList(RefListDialect::DesignerItem)),
            fc::F_DEFAULT_FORM => fp(p!("DefaultForm"), Codec::PlainText),
            fc::F_AUXILIARY_FORM => fp(p!("AuxiliaryForm"), Codec::PlainText),
            fc::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            fc::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            fc::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["FilterCriterion", "ChildObjects"];
        let (tag, bare) = match collection {
            "Form" => ("Form", true),
            "Command" => ("Command", false),
            _ => return None,
        };
        Some(ChildLocus { container: cont, child_tag: tag, props_wrapped: !bare, name_tag: "Name", bare_ref: bare })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        fc_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // ВСЕ Designer-FilterCriterion несут `<ChildObjects>` (сверено ERP 10/10 — минимум
        // самозакрывающийся `<ChildObjects/>` у объектов без форм/команд).
        true
    }
}

// ===== Command (props-wrapped, = Report.Command) =====
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

// FormRef — bare-ссылка (LocusMap не проецирует свойств).
pub struct DesignerFormRef;
impl LocusMap for DesignerFormRef {
    fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
        None
    }
}

fn fc_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Form", child_spec: filter_criterion_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Command", child_spec: filter_criterion_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("FilterCriterion", filter_criterion(), &DesignerFilterCriterion, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("FilterCriterion", filter_criterion(), &DesignerFilterCriterion, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/FilterCriterion. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "FilterCriterion",
    read,
    write,
    corpus_subpath: "coverage/designer/s3_bizproc/FilterCriteria",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
