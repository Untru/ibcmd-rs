//! EDT-проекция вида `CommonAttribute` (зеркало `core/spec/metadata/common_attribute.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Плоские дети корня `<mdclass:CommonAttribute>`. EDT эмитит РАЗРЕЖЁННО (дефолты
//! опущены), кроме always-present host'ов `type`/`minValue`/`maxValue`/`fillValue`.
//!
//! Физический порядок свойств (`ROOT_ORDER`) сверен по корпусам SSL (7 объектов) + coverage
//! (по одному не-дефолтному choice/fill-полю на объект): в EDT choice/fill-checking-кластер
//! (`fillChecking`/`choiceFoldersAndItems`/`quickChoice`/`createOnInput`/…) идёт ДО
//! `dataHistory`; `dataHistory` физически ДО `fillValue` (`fillFromFillingValue` между ними);
//! `choiceHistoryOnInput`/`content` (состав) — ПОСЛЕ `fillValue` (value-кластера).
//!
//! Корень несёт `xmlns:xsi`+`xmlns:core` (нужны Value-кодеку: `xsi:type="core:UndefinedValue"`).

use formats_xml::common_attribute_content::CommonAttributeContentDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, TypeDialect, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::common_attribute as ca;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

/// Карта проекции EDT для `CommonAttribute`.
pub struct EdtCommonAttribute;

impl LocusMap for EdtCommonAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ca::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            ca::F_COMMENT => fp(&["comment"], Codec::PlainText),
            ca::F_TYPE => fp(&["type"], Codec::Type(TypeDialect::Edt)),
            ca::F_PASSWORD_MODE => fp(&["passwordMode"], Codec::BoolPresence),
            ca::F_FORMAT => fp(&["format"], Codec::LocalizedKeyVal),
            ca::F_EDIT_FORMAT => fp(&["editFormat"], Codec::LocalizedKeyVal),
            ca::F_TOOL_TIP => fp(&["toolTip"], Codec::LocalizedKeyVal),
            ca::F_MARK_NEGATIVES => fp(&["markNegatives"], Codec::BoolPresence),
            ca::F_MASK => fp(&["mask"], Codec::PlainText),
            ca::F_MULTI_LINE => fp(&["multiLine"], Codec::BoolPresence),
            ca::F_EXTENDED_EDIT => fp(&["extendedEdit"], Codec::BoolPresence),
            ca::F_MIN_VALUE => fp(&["minValue"], Codec::Value(ValueDialect::Edt)),
            ca::F_MAX_VALUE => fp(&["maxValue"], Codec::Value(ValueDialect::Edt)),
            ca::F_FILL_FROM_FILLING_VALUE => fp(&["fillFromFillingValue"], Codec::BoolPresence),
            ca::F_FILL_VALUE => fp(&["fillValue"], Codec::Value(ValueDialect::Edt)),
            ca::F_FILL_CHECKING => fp(&["fillChecking"], Codec::EnumText),
            ca::F_CHOICE_FOLDERS_AND_ITEMS => fp(&["choiceFoldersAndItems"], Codec::EnumText),
            // choiceParameterLinks/choiceParameters — списки (в корпусе SSL пусты → List([]));
            // те же кодеки, что у Catalog.Attribute (List-value-kind).
            ca::F_CHOICE_PARAMETER_LINKS => fp(
                &["choiceParameterLinks"],
                Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Edt),
            ),
            ca::F_CHOICE_PARAMETERS => fp(
                &["choiceParameters"],
                Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Edt),
            ),
            ca::F_QUICK_CHOICE => fp(&["quickChoice"], Codec::EnumText),
            ca::F_CREATE_ON_INPUT => fp(&["createOnInput"], Codec::EnumText),
            ca::F_CHOICE_FORM => fp(&["choiceForm"], Codec::PlainText),
            ca::F_LINK_BY_TYPE => fp(&["linkByType"], Codec::PlainText),
            ca::F_CHOICE_HISTORY_ON_INPUT => fp(&["choiceHistoryOnInput"], Codec::EnumText),
            ca::F_CONTENT => fp(&["content"], Codec::CommonAttributeContent(CommonAttributeContentDialect::Edt)),
            ca::F_AUTO_USE => fp(&["autoUse"], Codec::EnumText),
            ca::F_DATA_SEPARATION => fp(&["dataSeparation"], Codec::EnumText),
            ca::F_SEPARATED_DATA_USE => fp(&["separatedDataUse"], Codec::EnumText),
            ca::F_DATA_SEPARATION_VALUE => fp(&["dataSeparationValue"], Codec::PlainText),
            ca::F_DATA_SEPARATION_USE => fp(&["dataSeparationUse"], Codec::PlainText),
            ca::F_CONDITIONAL_SEPARATION => fp(&["conditionalSeparation"], Codec::PlainText),
            ca::F_USERS_SEPARATION => fp(&["usersSeparation"], Codec::EnumText),
            ca::F_AUTHENTICATION_SEPARATION => fp(&["authenticationSeparation"], Codec::EnumText),
            ca::F_CONFIGURATION_EXTENSIONS_SEPARATION => {
                fp(&["configurationExtensionsSeparation"], Codec::EnumText)
            }
            ca::F_INDEXING => fp(&["indexing"], Codec::EnumText),
            ca::F_FULL_TEXT_SEARCH => fp(&["fullTextSearch"], Codec::EnumText),
            ca::F_DATA_HISTORY => fp(&["dataHistory"], Codec::EnumText),
            _ => return None,
        })
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

/// EDT физический порядок свойств корня CommonAttribute (сверено по корпусу SSL; поля
/// format/mask/choice*/linkByType дефолтны в корпусе — их позиция по EMF-супертипу).
static ROOT_ORDER: &[FieldId] = &[
    ca::F_SYNONYM,
    ca::F_COMMENT,
    ca::F_TYPE,
    ca::F_PASSWORD_MODE,
    ca::F_FORMAT,
    ca::F_EDIT_FORMAT,
    ca::F_TOOL_TIP,
    ca::F_MARK_NEGATIVES,
    ca::F_MASK,
    ca::F_MULTI_LINE,
    ca::F_EXTENDED_EDIT,
    ca::F_MIN_VALUE,
    ca::F_MAX_VALUE,
    // EDT физический порядок (сверено по coverage-корпусу, по одному не-дефолтному полю на
    // объект): choice/fill-checking-кластер идёт ДО `dataHistory`; `dataHistory` ДО
    // `fillValue`; `choiceHistoryOnInput`/`content` — ПОСЛЕ `fillValue` (наблюдения:
    // fillChecking/choiceFoldersAndItems/quickChoice/createOnInput < dataHistory < fillValue
    // < choiceHistoryOnInput; dataHistory < fillFromFillingValue < fillValue).
    ca::F_FILL_CHECKING,
    ca::F_CHOICE_FOLDERS_AND_ITEMS,
    ca::F_CHOICE_PARAMETER_LINKS,
    ca::F_CHOICE_PARAMETERS,
    ca::F_QUICK_CHOICE,
    ca::F_CREATE_ON_INPUT,
    ca::F_CHOICE_FORM,
    ca::F_LINK_BY_TYPE,
    ca::F_DATA_HISTORY,
    ca::F_FILL_FROM_FILLING_VALUE,
    ca::F_FILL_VALUE,
    ca::F_CHOICE_HISTORY_ON_INPUT,
    ca::F_CONTENT,
    ca::F_AUTO_USE,
    ca::F_DATA_SEPARATION,
    ca::F_SEPARATED_DATA_USE,
    ca::F_DATA_SEPARATION_VALUE,
    ca::F_DATA_SEPARATION_USE,
    ca::F_CONDITIONAL_SEPARATION,
    ca::F_USERS_SEPARATION,
    ca::F_AUTHENTICATION_SEPARATION,
    ca::F_CONFIGURATION_EXTENSIONS_SEPARATION,
    ca::F_INDEXING,
    ca::F_FULL_TEXT_SEARCH,
];

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_attribute::common_attribute;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonAttribute", common_attribute(), &EdtCommonAttribute, bytes)
        .map_err(|e| e.to_string())
}

fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonAttribute", common_attribute(), &EdtCommonAttribute, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/CommonAttribute. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "CommonAttribute",
    read,
    write,
    corpus_subpath: "coverage/edt/s4_common/src/CommonAttributes",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
