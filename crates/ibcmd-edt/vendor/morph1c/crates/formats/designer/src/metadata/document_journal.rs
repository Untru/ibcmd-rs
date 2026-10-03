//! Designer-проекция вида `DocumentJournal` + его дочерних видов (зеркало
//! `core/spec/metadata/document_journal*.rs`, ARCHITECTURE.md §5; child-objects substrate).
//! ТОЛЬКО размещение/кодировка — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО
//! ЖЕ спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5), включая детей. ИСКЛЮЧЕНИЕ:
//! `help` — EDT-only (Designer `.cf` его НЕ несёт) → Designer-проекция его НЕ проецирует;
//! поле X-исключено (`x_ignore`).
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<DocumentJournal>/<Properties>`, DENSE. Дети
//! Column/Command — `<ChildObjects>`+`<Properties>` (props_wrapped=true); Forms/Templates —
//! BARE-ссылки `<Form>Имя</Form>`.

use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::std_attrs_generic::GenDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::{EntitySpec, StdAttrsVariant};

use morph1c_core::spec::metadata::document_journal as dj;
use morph1c_core::spec::metadata::document_journal::document_journal;
use morph1c_core::spec::metadata::document_journal_column as col;
use morph1c_core::spec::metadata::document_journal_column::document_journal_column;
use morph1c_core::spec::metadata::document_journal_command as cmd;
use morph1c_core::spec::metadata::document_journal_command::document_journal_command;
use morph1c_core::spec::metadata::document_journal_form_ref::document_journal_form_ref;
use morph1c_core::spec::metadata::document_journal_template_ref::document_journal_template_ref;

const fn elem(path: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path, ns }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path, ""), codec }
}

// Пути от корня <MetaDataObject> через <DocumentJournal>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["DocumentJournal", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerDocumentJournal;

impl LocusMap for DesignerDocumentJournal {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            dj::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            dj::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            dj::F_DEFAULT_FORM => fp(p!("DefaultForm"), Codec::PlainText),
            dj::F_AUXILIARY_FORM => fp(p!("AuxiliaryForm"), Codec::PlainText),
            dj::F_USE_STANDARD_COMMANDS => fp(p!("UseStandardCommands"), Codec::BoolText),
            dj::F_REGISTERED_DOCUMENTS => fp(p!("RegisteredDocuments"), Codec::RefList(RefListDialect::DesignerItem)),
            dj::F_INCLUDE_HELP_IN_CONTENTS => fp(p!("IncludeHelpInContents"), Codec::BoolText),
            dj::F_STANDARD_ATTRIBUTES => fp(
                p!("StandardAttributes"),
                Codec::StdAttrs {
                    dialect: GenDialect::Designer,
                    decl: &dj::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            dj::F_LIST_PRESENTATION => fp(p!("ListPresentation"), Codec::LocalizedV8),
            dj::F_EXTENDED_LIST_PRESENTATION => fp(p!("ExtendedListPresentation"), Codec::LocalizedV8),
            dj::F_EXPLANATION => fp(p!("Explanation"), Codec::LocalizedV8),
            // help — EDT-only (Designer не несёт): not projected.
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["DocumentJournal", "ChildObjects"];
        let (tag, bare) = match collection {
            "Column" => ("Column", false),
            "Command" => ("Command", false),
            "Form" => ("Form", true),
            "Template" => ("Template", true),
            _ => return None,
        };
        Some(ChildLocus { container: cont, child_tag: tag, props_wrapped: !bare, name_tag: "Name", bare_ref: bare })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        dj_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // ВСЕ Designer-DocumentJournal несут `<ChildObjects>` (≥1 Column в корпусе 2/2).
        true
    }
}

// ===== Column (графа журнала) =====
pub struct DesignerColumn;
impl LocusMap for DesignerColumn {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            col::F_SYNONYM => fp(&["Synonym"], Codec::LocalizedV8),
            col::F_COMMENT => fp(&["Comment"], Codec::PlainText),
            col::F_INDEXING => fp(&["Indexing"], Codec::EnumText),
            col::F_REFERENCES => fp(&["References"], Codec::RefList(RefListDialect::DesignerItem)),
            _ => return None,
        })
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

fn dj_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Column", child_spec: document_journal_column(), child_map: &DesignerColumn },
            ChildBinding { collection: "Form", child_spec: document_journal_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: document_journal_template_ref(), child_map: &DesignerTemplateRef },
            ChildBinding { collection: "Command", child_spec: document_journal_command(), child_map: &DesignerCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("DocumentJournal", spec(), &DesignerDocumentJournal, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("DocumentJournal", spec(), &DesignerDocumentJournal, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    document_journal()
}

/// Строка R+X-харнесса Designer/DocumentJournal. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "DocumentJournal",
    read,
    write,
    corpus_subpath: "coverage/designer/s1_core/DocumentJournals",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
