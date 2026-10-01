//! Designer-проекция вида `CommonCommand` (зеркало `core/spec/metadata/common_command.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Designer DENSE — все поля present всегда (дефолты эмитятся: `<ToolTip/>`,
//! `<Picture/>`, `<Shortcut/>`, `<CommandParameterType/>`, `<ParameterUseMode>Single</…>` …).
//! Порядок = спек-порядок (DENSE), поэтому `field_emit_order` не нужен.
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — от КОРНЯ через `<CommonCommand>/<Properties>`:
//! * `group`/`representation`/`parameterUseMode`/`onMainServerUnavalableBehavior` → `EnumText`;
//! * `commandParameterType` → `Type(Designer)` (пустой → `<CommandParameterType/>`);
//! * `modifiesData`/`includeHelpInContents` → `BoolText`;
//! * `synonym`/`toolTip` → `LocalizedV8`; `comment`/`shortcut` → `PlainText`;
//! * `picture` → `PictureRef(Designer)` (пустой → `<Picture/>`).

use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::common_command as cc;

/// `PropElement{path, ns:""}` от корня (хелпер таблицы).
fn fp(tail: &'static str, codec: Codec) -> FieldProjection {
    FieldProjection::new(XmlLocus::PropElement { path: path_for(tail), ns: "" }, codec)
}

/// Полный путь `<CommonCommand>/<Properties>/<tail>` (статическая таблица).
fn path_for(tail: &'static str) -> &'static [&'static str] {
    match tail {
        "Synonym" => &["CommonCommand", "Properties", "Synonym"],
        "Comment" => &["CommonCommand", "Properties", "Comment"],
        "Group" => &["CommonCommand", "Properties", "Group"],
        "Representation" => &["CommonCommand", "Properties", "Representation"],
        "ToolTip" => &["CommonCommand", "Properties", "ToolTip"],
        "Picture" => &["CommonCommand", "Properties", "Picture"],
        "Shortcut" => &["CommonCommand", "Properties", "Shortcut"],
        "IncludeHelpInContents" => &["CommonCommand", "Properties", "IncludeHelpInContents"],
        "CommandParameterType" => &["CommonCommand", "Properties", "CommandParameterType"],
        "ParameterUseMode" => &["CommonCommand", "Properties", "ParameterUseMode"],
        "ModifiesData" => &["CommonCommand", "Properties", "ModifiesData"],
        "OnMainServerUnavalableBehavior" => {
            &["CommonCommand", "Properties", "OnMainServerUnavalableBehavior"]
        }
        _ => unreachable!("unknown CommonCommand property tail"),
    }
}

/// Карта проекции Designer для `CommonCommand`.
pub struct DesignerCommonCommand;

impl LocusMap for DesignerCommonCommand {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cc::F_SYNONYM => fp("Synonym", Codec::LocalizedV8),
            cc::F_COMMENT => fp("Comment", Codec::PlainText),
            cc::F_GROUP => fp("Group", Codec::EnumText),
            cc::F_REPRESENTATION => fp("Representation", Codec::EnumText),
            cc::F_TOOLTIP => fp("ToolTip", Codec::LocalizedV8),
            cc::F_PICTURE => {
                fp("Picture", Codec::PictureRef(formats_xml::picture::PictureDialect::Designer))
            }
            cc::F_SHORTCUT => fp("Shortcut", Codec::PlainText),
            cc::F_INCLUDE_HELP => fp("IncludeHelpInContents", Codec::BoolText),
            cc::F_COMMAND_PARAMETER_TYPE => {
                fp("CommandParameterType", Codec::Type(formats_xml::TypeDialect::Designer))
            }
            cc::F_PARAMETER_USE_MODE => fp("ParameterUseMode", Codec::EnumText),
            cc::F_MODIFIES_DATA => fp("ModifiesData", Codec::BoolText),
            cc::F_ON_MAIN_SERVER_UNAVALABLE => {
                fp("OnMainServerUnavalableBehavior", Codec::EnumText)
            }
            _ => return None,
        })
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_command::common_command;

/// R-read для харнесса: `.xml`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonCommand", common_command(), &DesignerCommonCommand, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonCommand", common_command(), &DesignerCommonCommand, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/CommonCommand. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "CommonCommand",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/CommonCommands",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
