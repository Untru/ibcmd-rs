//! EDT-проекция вида `CommonCommand` (зеркало `core/spec/metadata/common_command.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! EDT физический порядок тегов РАСХОДИТСЯ с каноническим (Designer DENSE) порядком спека
//! (как у `Catalog.Command`): synonym, comment, group, commandParameterType,
//! parameterUseMode, modifiesData, representation, toolTip, picture, shortcut,
//! includeHelpInContents, onMainServerUnavalableBehavior — задаётся `field_emit_order`.
//! Read позиционно-независим (по локусу); это лишь WRITE-хук (§1.6/§3.2).
//!
//! Карта `FieldId → (XmlLocus, Codec)` (EDT SPARSE — дефолты опущены):
//! * `group`/`representation`/`parameterUseMode`/`onMainServerUnavalableBehavior` → `EnumText`;
//! * `commandParameterType` → `Type(Edt)`;
//! * `modifiesData`/`includeHelpInContents` → `BoolPresence`;
//! * `toolTip`/`synonym` → `LocalizedKeyVal`; `comment`/`shortcut` → `PlainText`;
//! * `picture` → `PictureRef(Edt)` (xsi+core root-ns условно — present iff picture непуст).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`).

use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::common_command as cc;

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection::new(XmlLocus::PropElement { path: tag, ns: "" }, codec)
}

/// Карта проекции EDT для `CommonCommand`.
pub struct EdtCommonCommand;

impl LocusMap for EdtCommonCommand {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cc::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            cc::F_COMMENT => fp(&["comment"], Codec::PlainText),
            cc::F_GROUP => fp(&["group"], Codec::EnumText),
            cc::F_REPRESENTATION => fp(&["representation"], Codec::EnumText),
            cc::F_TOOLTIP => fp(&["toolTip"], Codec::LocalizedKeyVal),
            cc::F_PICTURE => {
                fp(&["picture"], Codec::PictureRef(formats_xml::picture::PictureDialect::Edt))
            }
            cc::F_SHORTCUT => fp(&["shortcut"], Codec::PlainText),
            cc::F_INCLUDE_HELP => fp(&["includeHelpInContents"], Codec::BoolPresence),
            cc::F_COMMAND_PARAMETER_TYPE => {
                fp(&["commandParameterType"], Codec::Type(formats_xml::TypeDialect::Edt))
            }
            cc::F_PARAMETER_USE_MODE => fp(&["parameterUseMode"], Codec::EnumText),
            cc::F_MODIFIES_DATA => fp(&["modifiesData"], Codec::BoolPresence),
            cc::F_ON_MAIN_SERVER_UNAVALABLE => {
                fp(&["onMainServerUnavalableBehavior"], Codec::EnumText)
            }
            _ => return None,
        })
    }

    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(EDT_ORDER)
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }

    fn root_extra_namespaces_optional(&self) -> bool {
        // xsi+core объявляются на корне команды ЛИШЬ при непустом <picture>
        // (его `xsi:type="core:PictureRef"` требует обоих ns). Сверено по корпусу SSL:
        // 18/63 объявляют ns ⟺ 18/63 несут непустую картинку.
        true
    }

    fn root_extra_namespaces_present(&self, obj: &morph1c_core::ir::MetadataObject) -> bool {
        obj.properties.iter().any(|(id, v)| {
            *id == cc::F_PICTURE
                && matches!(v, morph1c_core::ir::value::PropertyValue::Str(s) if !s.is_empty())
        })
    }
}

/// EDT-физический порядок тегов (расходится с каноническим спек-порядком).
static EDT_ORDER: &[FieldId] = &[
    cc::F_SYNONYM,
    cc::F_COMMENT,
    cc::F_GROUP,
    cc::F_COMMAND_PARAMETER_TYPE,
    cc::F_PARAMETER_USE_MODE,
    cc::F_MODIFIES_DATA,
    cc::F_REPRESENTATION,
    cc::F_TOOLTIP,
    cc::F_PICTURE,
    cc::F_SHORTCUT,
    cc::F_INCLUDE_HELP,
    cc::F_ON_MAIN_SERVER_UNAVALABLE,
];

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_command::common_command;

/// R-read для харнесса: `.mdo`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonCommand", common_command(), &EdtCommonCommand, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonCommand", common_command(), &EdtCommonCommand, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/CommonCommand. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "CommonCommand",
    read,
    write,
    corpus_subpath: "coverage/edt/s4_common/src/CommonCommands",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
