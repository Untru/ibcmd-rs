//! Designer-проекция вида `CommonAttribute` (зеркало `core/spec/metadata/common_attribute.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Иной синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от корня `<MetaDataObject>` через
//! `<CommonAttribute>/<Properties>`, DENSE (все свойства present, даже дефолты; порядок =
//! канонический порядок спека = Designer DENSE):
//! * локализованные → `LocalizedV8` (пустые → self-closing `<Tag/>`);
//! * `comment`/`mask`/`choice*`/`linkByType`/dataSeparation-refs → `PlainText` (пустые → `<Tag/>`);
//! * bool'ы → `BoolText` (`<Tag>true|false</Tag>`); enum'ы → `EnumText`;
//! * `type` → `Type(Designer)`; `minValue`/`maxValue`/`fillValue` → `Value(Designer)`;
//! * `content` → `CommonAttributeContent(Designer)` — контейнер `<Content>` с `<xr:Item>`'ами.
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/`v8:Type`/xsi-атрибуты/`xr:Item`) несут v8/xsi/xr — их разбирают кодеки.

use formats_xml::common_attribute_content::CommonAttributeContentDialect;
use formats_xml::value_codec::ValueDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, TypeDialect, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::common_attribute as ca;

/// Плоский `PropElement{path, ns:""}` под `<CommonAttribute>/<Properties>/<Tag>`.
const fn prop(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: XmlLocus::PropElement { path: tag, ns: "" }, codec }
}

const P_SYNONYM: &[&str] = &["CommonAttribute", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["CommonAttribute", "Properties", "Comment"];
const P_TYPE: &[&str] = &["CommonAttribute", "Properties", "Type"];
const P_PASSWORD_MODE: &[&str] = &["CommonAttribute", "Properties", "PasswordMode"];
const P_FORMAT: &[&str] = &["CommonAttribute", "Properties", "Format"];
const P_EDIT_FORMAT: &[&str] = &["CommonAttribute", "Properties", "EditFormat"];
const P_TOOL_TIP: &[&str] = &["CommonAttribute", "Properties", "ToolTip"];
const P_MARK_NEGATIVES: &[&str] = &["CommonAttribute", "Properties", "MarkNegatives"];
const P_MASK: &[&str] = &["CommonAttribute", "Properties", "Mask"];
const P_MULTI_LINE: &[&str] = &["CommonAttribute", "Properties", "MultiLine"];
const P_EXTENDED_EDIT: &[&str] = &["CommonAttribute", "Properties", "ExtendedEdit"];
const P_MIN_VALUE: &[&str] = &["CommonAttribute", "Properties", "MinValue"];
const P_MAX_VALUE: &[&str] = &["CommonAttribute", "Properties", "MaxValue"];
const P_FILL_FROM_FILLING_VALUE: &[&str] = &["CommonAttribute", "Properties", "FillFromFillingValue"];
const P_FILL_VALUE: &[&str] = &["CommonAttribute", "Properties", "FillValue"];
const P_FILL_CHECKING: &[&str] = &["CommonAttribute", "Properties", "FillChecking"];
const P_CHOICE_FOLDERS_AND_ITEMS: &[&str] = &["CommonAttribute", "Properties", "ChoiceFoldersAndItems"];
const P_CHOICE_PARAMETER_LINKS: &[&str] = &["CommonAttribute", "Properties", "ChoiceParameterLinks"];
const P_CHOICE_PARAMETERS: &[&str] = &["CommonAttribute", "Properties", "ChoiceParameters"];
const P_QUICK_CHOICE: &[&str] = &["CommonAttribute", "Properties", "QuickChoice"];
const P_CREATE_ON_INPUT: &[&str] = &["CommonAttribute", "Properties", "CreateOnInput"];
const P_CHOICE_FORM: &[&str] = &["CommonAttribute", "Properties", "ChoiceForm"];
const P_LINK_BY_TYPE: &[&str] = &["CommonAttribute", "Properties", "LinkByType"];
const P_CHOICE_HISTORY_ON_INPUT: &[&str] = &["CommonAttribute", "Properties", "ChoiceHistoryOnInput"];
const P_CONTENT: &[&str] = &["CommonAttribute", "Properties", "Content"];
const P_AUTO_USE: &[&str] = &["CommonAttribute", "Properties", "AutoUse"];
const P_DATA_SEPARATION: &[&str] = &["CommonAttribute", "Properties", "DataSeparation"];
const P_SEPARATED_DATA_USE: &[&str] = &["CommonAttribute", "Properties", "SeparatedDataUse"];
const P_DATA_SEPARATION_VALUE: &[&str] = &["CommonAttribute", "Properties", "DataSeparationValue"];
const P_DATA_SEPARATION_USE: &[&str] = &["CommonAttribute", "Properties", "DataSeparationUse"];
const P_CONDITIONAL_SEPARATION: &[&str] = &["CommonAttribute", "Properties", "ConditionalSeparation"];
const P_USERS_SEPARATION: &[&str] = &["CommonAttribute", "Properties", "UsersSeparation"];
const P_AUTHENTICATION_SEPARATION: &[&str] = &["CommonAttribute", "Properties", "AuthenticationSeparation"];
const P_CONFIGURATION_EXTENSIONS_SEPARATION: &[&str] =
    &["CommonAttribute", "Properties", "ConfigurationExtensionsSeparation"];
const P_INDEXING: &[&str] = &["CommonAttribute", "Properties", "Indexing"];
const P_FULL_TEXT_SEARCH: &[&str] = &["CommonAttribute", "Properties", "FullTextSearch"];
const P_DATA_HISTORY: &[&str] = &["CommonAttribute", "Properties", "DataHistory"];

/// Карта проекции Designer для `CommonAttribute`.
pub struct DesignerCommonAttribute;

impl LocusMap for DesignerCommonAttribute {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ca::F_SYNONYM => prop(P_SYNONYM, Codec::LocalizedV8),
            ca::F_COMMENT => prop(P_COMMENT, Codec::PlainText),
            ca::F_TYPE => prop(P_TYPE, Codec::Type(TypeDialect::Designer)),
            ca::F_PASSWORD_MODE => prop(P_PASSWORD_MODE, Codec::BoolText),
            ca::F_FORMAT => prop(P_FORMAT, Codec::LocalizedV8),
            ca::F_EDIT_FORMAT => prop(P_EDIT_FORMAT, Codec::LocalizedV8),
            ca::F_TOOL_TIP => prop(P_TOOL_TIP, Codec::LocalizedV8),
            ca::F_MARK_NEGATIVES => prop(P_MARK_NEGATIVES, Codec::BoolText),
            ca::F_MASK => prop(P_MASK, Codec::PlainText),
            ca::F_MULTI_LINE => prop(P_MULTI_LINE, Codec::BoolText),
            ca::F_EXTENDED_EDIT => prop(P_EXTENDED_EDIT, Codec::BoolText),
            ca::F_MIN_VALUE => prop(P_MIN_VALUE, Codec::Value(ValueDialect::Designer)),
            ca::F_MAX_VALUE => prop(P_MAX_VALUE, Codec::Value(ValueDialect::Designer)),
            ca::F_FILL_FROM_FILLING_VALUE => prop(P_FILL_FROM_FILLING_VALUE, Codec::BoolText),
            ca::F_FILL_VALUE => prop(P_FILL_VALUE, Codec::Value(ValueDialect::Designer)),
            ca::F_FILL_CHECKING => prop(P_FILL_CHECKING, Codec::EnumText),
            ca::F_CHOICE_FOLDERS_AND_ITEMS => prop(P_CHOICE_FOLDERS_AND_ITEMS, Codec::EnumText),
            ca::F_CHOICE_PARAMETER_LINKS => prop(
                P_CHOICE_PARAMETER_LINKS,
                Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Designer),
            ),
            ca::F_CHOICE_PARAMETERS => prop(
                P_CHOICE_PARAMETERS,
                Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Designer),
            ),
            ca::F_QUICK_CHOICE => prop(P_QUICK_CHOICE, Codec::EnumText),
            ca::F_CREATE_ON_INPUT => prop(P_CREATE_ON_INPUT, Codec::EnumText),
            ca::F_CHOICE_FORM => prop(P_CHOICE_FORM, Codec::PlainText),
            ca::F_LINK_BY_TYPE => prop(P_LINK_BY_TYPE, Codec::PlainText),
            ca::F_CHOICE_HISTORY_ON_INPUT => prop(P_CHOICE_HISTORY_ON_INPUT, Codec::EnumText),
            ca::F_CONTENT => {
                prop(P_CONTENT, Codec::CommonAttributeContent(CommonAttributeContentDialect::Designer))
            }
            ca::F_AUTO_USE => prop(P_AUTO_USE, Codec::EnumText),
            ca::F_DATA_SEPARATION => prop(P_DATA_SEPARATION, Codec::EnumText),
            ca::F_SEPARATED_DATA_USE => prop(P_SEPARATED_DATA_USE, Codec::EnumText),
            ca::F_DATA_SEPARATION_VALUE => prop(P_DATA_SEPARATION_VALUE, Codec::PlainText),
            ca::F_DATA_SEPARATION_USE => prop(P_DATA_SEPARATION_USE, Codec::PlainText),
            ca::F_CONDITIONAL_SEPARATION => prop(P_CONDITIONAL_SEPARATION, Codec::PlainText),
            ca::F_USERS_SEPARATION => prop(P_USERS_SEPARATION, Codec::EnumText),
            ca::F_AUTHENTICATION_SEPARATION => prop(P_AUTHENTICATION_SEPARATION, Codec::EnumText),
            ca::F_CONFIGURATION_EXTENSIONS_SEPARATION => {
                prop(P_CONFIGURATION_EXTENSIONS_SEPARATION, Codec::EnumText)
            }
            ca::F_INDEXING => prop(P_INDEXING, Codec::EnumText),
            ca::F_FULL_TEXT_SEARCH => prop(P_FULL_TEXT_SEARCH, Codec::EnumText),
            ca::F_DATA_HISTORY => prop(P_DATA_HISTORY, Codec::EnumText),
            _ => return None,
        })
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_attribute::common_attribute;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonAttribute", common_attribute(), &DesignerCommonAttribute, bytes)
        .map_err(|e| e.to_string())
}

fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonAttribute", common_attribute(), &DesignerCommonAttribute, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/CommonAttribute. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "CommonAttribute",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/CommonAttributes",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
