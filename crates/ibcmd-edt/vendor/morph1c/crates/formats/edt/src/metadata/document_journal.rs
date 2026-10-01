//! EDT-проекция вида `DocumentJournal` + его дочерних видов (зеркало
//! `core/spec/metadata/document_journal*.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Переиспользует Document/
//! ExchangePlan-субстрат (ref-list/value/picture/help/usePurposes-кодеки, children-
//! рекурсию, FormRef/TemplateRef-стабы, Command) + DocumentJournal-дельту
//! (std_attrs_document_journal, registeredDocuments, Column-коллекция с references).
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дети Column/Command — inline (props_wrapped=false);
//! Forms/Templates — inline-СТАБЫ. EDT-only `help` — X-исключён. EDT эмитит РАЗРЕЖЁННО;
//! физический порядок свойств — `field_emit_order` (сверено по корпусу).

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
use morph1c_core::spec::metadata::document_journal_form_ref as fref;
use morph1c_core::spec::metadata::document_journal_form_ref::document_journal_form_ref;
use morph1c_core::spec::metadata::document_journal_template_ref as tref;
use morph1c_core::spec::metadata::document_journal_template_ref::document_journal_template_ref;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ DocumentJournal =====
pub struct EdtDocumentJournal;

impl LocusMap for EdtDocumentJournal {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            dj::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            dj::F_COMMENT => fp(&["comment"], Codec::PlainText),
            dj::F_DEFAULT_FORM => fp(&["defaultForm"], Codec::PlainText),
            dj::F_AUXILIARY_FORM => fp(&["auxiliaryForm"], Codec::PlainText),
            dj::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            dj::F_REGISTERED_DOCUMENTS => fp(&["registeredDocuments"], Codec::RefList(RefListDialect::Edt)),
            dj::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            dj::F_STANDARD_ATTRIBUTES => fp(
                &["standardAttributes"],
                Codec::StdAttrs {
                    dialect: GenDialect::Edt,
                    decl: &dj::STD_ATTRS,
                    variant: StdAttrsVariant::Root,
                },
            ),
            dj::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            dj::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            dj::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            dj::F_HELP => fp(&["help"], Codec::HelpConst),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let tag = match collection {
            "Column" => "columns",
            "Command" => "commands",
            "Form" => "forms",
            "Template" => "templates",
            _ => return None,
        };
        Some(ChildLocus { container: &[], child_tag: tag, props_wrapped: false, name_tag: "name", bare_ref: false })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        dj_bindings()
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

/// EDT физический порядок свойств корня DocumentJournal (сверено 2/2 по корпусу):
/// synonym, defaultForm, useStandardCommands, registeredDocuments, help, standardAttributes,
/// listPresentation, extendedListPresentation. (comment/auxiliaryForm/includeHelpInContents/
/// explanation — дефолт-омиссии, попадают в хвост; в корпусе не встречаются ненулевыми.)
static ROOT_ORDER: &[FieldId] = &[
    dj::F_SYNONYM,
    dj::F_COMMENT,
    dj::F_DEFAULT_FORM,
    dj::F_AUXILIARY_FORM,
    dj::F_USE_STANDARD_COMMANDS,
    dj::F_REGISTERED_DOCUMENTS,
    dj::F_INCLUDE_HELP_IN_CONTENTS,
    dj::F_HELP,
    dj::F_STANDARD_ATTRIBUTES,
    dj::F_LIST_PRESENTATION,
    dj::F_EXTENDED_LIST_PRESENTATION,
    dj::F_EXPLANATION,
];

// ===== Column (графа журнала) =====
pub struct EdtColumn;
impl LocusMap for EdtColumn {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            col::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            // comment — считался Designer-only, но ERP витнессит его и в `.mdo` (witness:
            // `ВедомостиНаВыплатуЗарплаты`, графа `ПериодРегистрации` — <comment> между
            // <synonym> и <references>).
            col::F_COMMENT => fp(&["comment"], Codec::PlainText),
            col::F_INDEXING => fp(&["indexing"], Codec::EnumText),
            col::F_REFERENCES => fp(&["references"], Codec::RefList(RefListDialect::Edt)),
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(COLUMN_ORDER)
    }
}

/// EDT физический порядок свойств Column: synonym, comment, indexing, references (comment —
/// сразу после synonym; witness ERP `ВедомостиНаВыплатуЗарплаты.ПериодРегистрации`, позиция
/// = порядку спека/metamodel).
static COLUMN_ORDER: &[FieldId] = &[col::F_SYNONYM, col::F_COMMENT, col::F_INDEXING, col::F_REFERENCES];

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
            fref::F_USE_PURPOSES => fp(&["usePurposes"], Codec::RefList(formats_xml::ref_list::RefListDialect::Edt)),
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

/// `&'static` бинды child-видов корня DocumentJournal.
fn dj_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Column", child_spec: document_journal_column(), child_map: &EdtColumn },
            ChildBinding { collection: "Form", child_spec: document_journal_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: document_journal_template_ref(), child_map: &EdtTemplateRef },
            ChildBinding { collection: "Command", child_spec: document_journal_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("DocumentJournal", spec(), &EdtDocumentJournal, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("DocumentJournal", spec(), &EdtDocumentJournal, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    document_journal()
}

/// Строка R+X-харнесса EDT/DocumentJournal. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "DocumentJournal",
    read,
    write,
    corpus_subpath: "coverage/edt/s1_core/src/DocumentJournals",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
