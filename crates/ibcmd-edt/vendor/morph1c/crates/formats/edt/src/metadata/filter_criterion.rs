//! EDT-проекция вида `FilterCriterion` + его дочерних видов (зеркало
//! `core/spec/metadata/filter_criterion*.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Переиспользует СУЩЕСТВУЮЩИЙ
//! субстрат: `Type`-кодек, `RefList` (content, EDT — сиблинги `<content>Path</content>`),
//! FormRef (inline-стаб), Command (= Report.Command). Платформенный `<producedTypes>`
//! (List+Manager) фреймится каркасом коннектора (`PRODUCED_CATEGORIES`), не тут.
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. EDT эмитит РАЗРЕЖЁННО; физический порядок
//! present-свойств совпадает со спек-порядком (`comment` дефолтен → опущен), поэтому
//! `field_emit_order` у корня не нужен. Forms — inline-СТАБЫ; Commands — inline props.

use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;

use morph1c_core::spec::metadata::filter_criterion as fc;
use morph1c_core::spec::metadata::filter_criterion::filter_criterion;
use morph1c_core::spec::metadata::filter_criterion_command as cmd;
use morph1c_core::spec::metadata::filter_criterion_command::filter_criterion_command;
use morph1c_core::spec::metadata::filter_criterion_form_ref as fref;
use morph1c_core::spec::metadata::filter_criterion_form_ref::filter_criterion_form_ref;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ FilterCriterion =====
pub struct EdtFilterCriterion;

impl LocusMap for EdtFilterCriterion {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            fc::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            fc::F_COMMENT => fp(&["comment"], Codec::PlainText),
            fc::F_TYPE => fp(&["type"], Codec::Type(formats_xml::TypeDialect::Edt)),
            fc::F_USE_STANDARD_COMMANDS => fp(&["useStandardCommands"], Codec::BoolPresence),
            fc::F_CONTENT => fp(&["content"], Codec::RefList(RefListDialect::Edt)),
            fc::F_DEFAULT_FORM => fp(&["defaultForm"], Codec::PlainText),
            fc::F_AUXILIARY_FORM => fp(&["auxiliaryForm"], Codec::PlainText),
            fc::F_LIST_PRESENTATION => fp(&["listPresentation"], Codec::LocalizedKeyVal),
            fc::F_EXTENDED_LIST_PRESENTATION => fp(&["extendedListPresentation"], Codec::LocalizedKeyVal),
            fc::F_EXPLANATION => fp(&["explanation"], Codec::LocalizedKeyVal),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let (tag, name_tag) = match collection {
            "Form" => ("forms", "name"),
            "Command" => ("commands", "name"),
            _ => return None,
        };
        Some(ChildLocus { container: &[], child_tag: tag, props_wrapped: false, name_tag, bare_ref: false })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        fc_bindings()
    }
}

// ===== FormRef (EDT — полный inline-стаб) =====
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
            fref::F_EXTENDED_PRESENTATION => fp(&["extendedPresentation"], Codec::LocalizedKeyVal),
            _ => return None,
        })
    }
}

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
            // onMainServerUnavalableBehavior — Designer-only (EDT не несёт).
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

/// `&'static` бинды child-видов корня FilterCriterion.
fn fc_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Form", child_spec: filter_criterion_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Command", child_spec: filter_criterion_command(), child_map: &EdtCommand },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("FilterCriterion", filter_criterion(), &EdtFilterCriterion, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("FilterCriterion", filter_criterion(), &EdtFilterCriterion, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/FilterCriterion. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "FilterCriterion",
    read,
    write,
    corpus_subpath: "coverage/edt/s3_bizproc/src/FilterCriteria",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
